use std::{
    collections::HashSet,
    sync::{Arc, LazyLock},
};

use regex::Regex;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use serenity::{
    all::{Context, CreateEmbed, CreateEmbedFooter, CreateMessage, EventHandler, Message},
    async_trait,
};
use tracing::error;

use crate::config::get_config;


static MODRINTH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"https?://modrinth\.com/(?:mod|plugin|datapack|resourcepack|modpack|shader|project)/([A-Za-z0-9_-]+)",
    )
    .unwrap()
});

static CURSEFORGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://(?:www\.)?curseforge\.com/([^/\s]+)/([^/\s]+)/([A-Za-z0-9_-]+)").unwrap()
});

static LOADER_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ["Forge", "Fabric", "Quilt", "NeoForge", "LiteLoader", "Cauldron"].into_iter().collect()
});

enum DetectedUrl {
    Modrinth { slug: String },
    CurseForge { slug: String, category: String },
    OtherGame,
}

fn detect_urls(content: &str) -> Vec<DetectedUrl> {
    let mut results = Vec::new();
    for cap in MODRINTH_RE.captures_iter(content) {
        results.push(DetectedUrl::Modrinth { slug: cap[1].to_string() });
    }
    for cap in CURSEFORGE_RE.captures_iter(content) {
        if &cap[1] != "minecraft" {
            results.push(DetectedUrl::OtherGame);
        } else {
            results.push(DetectedUrl::CurseForge { slug: cap[3].to_string(), category: cap[2].to_string() });
        }
    }
    results
}



#[derive(Deserialize)]
struct ModrinthProject {
    title: String,
    description: Option<String>,
    slug: String,
    icon_url: Option<String>,
    downloads: u64,
    game_versions: Option<Vec<String>>,
    loaders: Option<Vec<String>>,
    published: Option<String>,
    updated: Option<String>,
}

#[derive(Deserialize)]
struct ModrinthMember {
    user: ModrinthUser,
}

#[derive(Deserialize)]
struct ModrinthUser {
    username: String,
}

#[derive(Deserialize)]
struct CfSearchResponse {
    data: Vec<CfMod>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    name: String,
    summary: Option<String>,
    download_count: f64,
    date_created: Option<String>,
    date_modified: Option<String>,
    links: Option<CfLinks>,
    logo: Option<CfLogo>,
    authors: Option<Vec<CfAuthor>>,
    latest_files: Option<Vec<CfFile>>,
    latest_files_indexes: Option<Vec<CfFileIndex>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLinks {
    website_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfLogo {
    thumbnail_url: Option<String>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct CfAuthor {
    name: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    game_versions: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfFileIndex {
    game_version: Option<String>,
}



struct ModData {
    title: String,
    description: String,
    url: String,
    icon_url: Option<String>,
    downloads: u64,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    published: Option<String>,
    updated: Option<String>,
    authors: Vec<(String, String)>,
}



type FetchError = Box<dyn std::error::Error + Send + Sync>;

fn category_to_class_id(category: &str) -> Option<&'static str> {
    match category {
        "mc-mods" => Some("6"),
        "texture-packs" => Some("12"),
        "worlds" => Some("17"),
        "modpacks" => Some("4471"),
        "customization" => Some("4546"),
        "bukkit-plugins" => Some("5"),
        "shaders" => Some("6552"),
        "mc-addons" => Some("4559"),
        _ => None,
    }
}

async fn fetch_modrinth(client: &HttpClient, slug: &str) -> Result<ModData, FetchError> {
    let base = "https://api.modrinth.com/v2";
    let ua = "valine_bot/1.0 (discord-bot)";

    let (proj_res, members_res) = tokio::try_join!(
        client.get(format!("{base}/project/{slug}")).header("User-Agent", ua).send(),
        client.get(format!("{base}/project/{slug}/members")).header("User-Agent", ua).send(),
    )?;

    let proj: ModrinthProject = proj_res.error_for_status()?.json().await?;
    let members: Vec<ModrinthMember> = members_res.error_for_status()?.json().await?;

    let mut game_versions: Vec<String> = proj
        .game_versions
        .unwrap_or_default()
        .into_iter()
        .filter(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()) && v.contains('.'))
        .collect();
    game_versions.sort_by(|a, b| b.cmp(a));

    let loaders = proj
        .loaders
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                None => s,
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect();

    let authors = members
        .into_iter()
        .take(5)
        .map(|m| (m.user.username.clone(), format!("https://modrinth.com/user/{}", m.user.username)))
        .collect();

    Ok(ModData {
        title: proj.title,
        description: truncate(proj.description.unwrap_or_default(), 300),
        url: format!("https://modrinth.com/project/{}", proj.slug),
        icon_url: proj.icon_url,
        downloads: proj.downloads,
        game_versions,
        loaders,
        published: proj.published,
        updated: proj.updated,
        authors,
    })
}

async fn fetch_curseforge(
    client: &HttpClient,
    api_key: &str,
    slug: &str,
    category: &str,
) -> Result<Option<ModData>, FetchError> {
    let mut req = client
        .get("https://api.curseforge.com/v1/mods/search")
        .header("x-api-key", api_key)
        .query(&[("gameId", "432"), ("slug", slug)]);
    if let Some(class_id) = category_to_class_id(category) {
        req = req.query(&[("classId", class_id)]);
    }
    let res: CfSearchResponse = req.send().await?.error_for_status()?.json().await?;

    let mod_data = match res.data.into_iter().next() {
        Some(m) => m,
        None => return Ok(None),
    };

    let mut version_set: HashSet<String> = HashSet::new();
    let mut loader_set: HashSet<String> = HashSet::new();

    for file in mod_data.latest_files.as_deref().unwrap_or(&[]) {
        for gv in file.game_versions.as_deref().unwrap_or(&[]) {
            if gv.chars().next().is_some_and(|c| c.is_ascii_digit()) && gv.contains('.') {
                version_set.insert(gv.clone());
            } else if LOADER_NAMES.contains(gv.as_str()) {
                loader_set.insert(gv.clone());
            }
        }
    }
    for idx in mod_data.latest_files_indexes.as_deref().unwrap_or(&[]) {
        if let Some(gv) = &idx.game_version {
            if gv.chars().next().is_some_and(|c| c.is_ascii_digit()) && gv.contains('.') {
                version_set.insert(gv.clone());
            }
        }
    }

    let mut game_versions: Vec<String> = version_set.into_iter().collect();
    game_versions.sort_by(|a, b| b.cmp(a));

    let loaders: Vec<String> = loader_set.into_iter().collect();

    let authors = mod_data
        .authors
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .take(5)
        .map(|a| (a.name.clone(), a.url.clone()))
        .collect();

    let fallback_url = format!("https://www.curseforge.com/minecraft/{category}/{slug}");

    Ok(Some(ModData {
        title: mod_data.name,
        description: truncate(mod_data.summary.unwrap_or_default(), 300),
        url: mod_data.links.and_then(|l| l.website_url).unwrap_or(fallback_url),
        icon_url: mod_data.logo.as_ref().and_then(|l| l.thumbnail_url.as_ref().or(l.url.as_ref())).cloned(),
        downloads: mod_data.download_count as u64,
        game_versions,
        loaders,
        published: mod_data.date_created,
        updated: mod_data.date_modified,
        authors,
    }))
}



fn build_embed(data: &ModData, site: &str) -> CreateEmbed {
    let color: u32 = if site == "Modrinth" { 0x1bd96a } else { 0xf16436 };

    let mut embed = CreateEmbed::new().color(color).title(&data.title).url(&data.url);

    if !data.description.is_empty() {
        embed = embed.description(&data.description);
    }
    if let Some(icon) = &data.icon_url {
        embed = embed.thumbnail(icon);
    }
    if !data.game_versions.is_empty() {
        embed = embed.field("Game Versions", fit_text(&data.game_versions.join(", ")), true);
    }
    if !data.loaders.is_empty() {
        embed = embed.field("Mod Loaders", data.loaders.join(", "), true);
    }
    embed = embed.field("Downloads", format_number(data.downloads), true);
    if let Some(d) = &data.published {
        embed = embed.field("Published", format_date(d), true);
    }
    if let Some(d) = &data.updated {
        embed = embed.field("Updated", format_date(d), true);
    }
    if !data.authors.is_empty() {
        let s = data.authors.iter().map(|(n, u)| format!("[{n}]({u})")).collect::<Vec<_>>().join(", ");
        embed = embed.field("Author", s, false);
    }
    embed.footer(CreateEmbedFooter::new(site))
}

fn error_embed(msg: &str) -> CreateEmbed {
    CreateEmbed::new().color(0xe74c3cu32).title("エラー").description(msg)
}

fn resolve_http_error(e: &reqwest::Error) -> &'static str {
    match e.status().map(|s| s.as_u16()) {
        Some(400..=499) => "URLが正しくないか、Modが見つかりませんでした。",
        Some(500..=599) => "サービス側でエラーが発生しました。しばらく後に試してください。",
        _ => "エラーが発生しました。",
    }
}



fn fit_text(s: &str) -> String {
    if s.len() <= 1024 { s.to_string() } else { format!("{}…", &s[..1023]) }
}

fn truncate(s: String, max: usize) -> String {
    if s.len() <= max { s } else { format!("{}…", &s[..max]) }
}

fn format_date(iso: &str) -> String {
    if iso.len() >= 10 {
        let p: Vec<&str> = iso[..10].split('-').collect();
        if p.len() == 3 {
            return format!("{}/{}/{}", p[0], p[1], p[2]);
        }
    }
    iso.to_string()
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}



pub struct Handler {
    http_client: Arc<HttpClient>,
}

impl Handler {
    pub fn new() -> Self {
        Self { http_client: Arc::new(HttpClient::new()) }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let config = get_config(&ctx).await;
        let mod_info = &config.mod_info;

        let allowed = if mod_info.mode == "channels" {
            mod_info.allowed_channel_ids.contains(&msg.channel_id)
        } else {
            true
        };
        if !allowed {
            return;
        }

        let detected = detect_urls(&msg.content);
        if detected.is_empty() {
            return;
        }

        for url in detected.into_iter().take(3) {
            let embed = match url {
                DetectedUrl::OtherGame => continue,
                DetectedUrl::Modrinth { slug } => match fetch_modrinth(&self.http_client, &slug).await {
                    Ok(data) => build_embed(&data, "Modrinth"),
                    Err(e) => {
                        error!("[Modrinth] {slug}: {e}");
                        let err_msg = e
                            .downcast_ref::<reqwest::Error>()
                            .map(resolve_http_error)
                            .unwrap_or("エラーが発生しました。");
                        error_embed(err_msg)
                    }
                },
                DetectedUrl::CurseForge { slug, category } => match &mod_info.curseforge_api_key {
                    None => error_embed("CurseForge APIキーが設定されていません。"),
                    Some(api_key) => match fetch_curseforge(&self.http_client, api_key, &slug, &category).await {
                        Ok(Some(data)) => build_embed(&data, "CurseForge"),
                        Ok(None) => error_embed("URLが正しくないか、Modが見つかりませんでした。"),
                        Err(e) => {
                            error!("[CurseForge] {slug}: {e}");
                            let err_msg = e
                                .downcast_ref::<reqwest::Error>()
                                .map(resolve_http_error)
                                .unwrap_or("エラーが発生しました。");
                            error_embed(err_msg)
                        }
                    },
                },
            };
            let _ = msg.channel_id.send_message(&ctx.http, CreateMessage::new().embed(embed)).await;
        }
    }
}
