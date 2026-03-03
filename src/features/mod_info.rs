use std::{
    collections::HashSet,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

use dashmap::DashMap;
use regex::Regex;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use serenity::{
    all::{
        ChannelId, Context, CreateAllowedMentions, CreateEmbed, CreateEmbedFooter, CreateMessage,
        EditMessage, EventHandler, GuildId, Message, MessageFlags, MessageId, MessageUpdateEvent,
    },
    async_trait,
};
use tracing::{error, warn};

use crate::config::get_config;

const MAX_AUTHORS: usize = 5;
const MAX_DESCRIPTION_CHARS: usize = 300;
const MAX_FIELD_CHARS: usize = 1024;
const MODRINTH_COLOR: u32 = 0x1bd96a;
const CURSEFORGE_COLOR: u32 = 0xf16436;
const ERROR_COLOR: u32 = 0xe74c3c;
const USER_AGENT: &str = "valine_bot/1.0 (discord-bot)";
const MINECRAFT_GAME_ID: &str = "432";
const HTTP_TIMEOUT_SECS: u64 = 10;
const CACHE_TTL_SECS: u64 = 300;
const REPLY_MAP_TTL_SECS: u64 = 3600;

static MODRINTH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"https?://modrinth\.com/(?:mod|plugin|datapack|resourcepack|modpack|shader|project)/([A-Za-z0-9_-]+)",
    )
    .unwrap()
});

static CURSEFORGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://(?:www\.)?curseforge\.com/([^/\s]+)/([^/\s]+)/([A-Za-z0-9_-]+)")
        .unwrap()
});

static STRIP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)```.*?```|`[^`]+`|<https?://[^\s>]+>").unwrap()
});

static LOADER_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ["Forge", "Fabric", "Quilt", "NeoForge", "LiteLoader", "Cauldron"]
        .into_iter()
        .collect()
});

enum DetectedUrl {
    Modrinth { slug: String },
    CurseForge { slug: String, category: String },
}

fn detect_urls(content: &str) -> Vec<DetectedUrl> {
    let cleaned = STRIP_RE.replace_all(content, " ");
    let mut results = Vec::new();
    for cap in MODRINTH_RE.captures_iter(&cleaned) {
        results.push(DetectedUrl::Modrinth {
            slug: cap[1].to_string(),
        });
    }
    for cap in CURSEFORGE_RE.captures_iter(&cleaned) {
        if &cap[1] != "minecraft" {
            continue;
        }
        results.push(DetectedUrl::CurseForge {
            slug: cap[3].to_string(),
            category: cap[2].to_string(),
        });
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

#[derive(Clone)]
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

fn truncate_str(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_bytes)
        .last()
        .unwrap_or(0);
    format!("{}…", &s[..end])
}

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

fn build_embed(data: &ModData, site: &str) -> CreateEmbed {
    let color: u32 = if site == "Modrinth" {
        MODRINTH_COLOR
    } else {
        CURSEFORGE_COLOR
    };

    let mut embed = CreateEmbed::new()
        .color(color)
        .title(&data.title)
        .url(&data.url);

    if !data.description.is_empty() {
        embed = embed.description(&data.description);
    }
    if let Some(icon) = &data.icon_url {
        embed = embed.thumbnail(icon);
    }
    if !data.game_versions.is_empty() {
        embed = embed.field(
            "Game Versions",
            truncate_str(&data.game_versions.join(", "), MAX_FIELD_CHARS),
            true,
        );
    }
    if !data.loaders.is_empty() {
        embed = embed.field(
            "Mod Loaders",
            truncate_str(&data.loaders.join(", "), MAX_FIELD_CHARS),
            true,
        );
    }
    embed = embed.field("Downloads", format_number(data.downloads), true);
    if let Some(d) = &data.published {
        embed = embed.field("Published", format_date(d), true);
    }
    if let Some(d) = &data.updated {
        embed = embed.field("Updated", format_date(d), true);
    }
    if !data.authors.is_empty() {
        let s = data
            .authors
            .iter()
            .map(|(n, u)| format!("[{n}]({u})"))
            .collect::<Vec<_>>()
            .join(", ");
        embed = embed.field("Author", s, false);
    }
    embed.footer(CreateEmbedFooter::new(site))
}

fn error_embed(msg: &str) -> CreateEmbed {
    CreateEmbed::new()
        .color(ERROR_COLOR)
        .title("エラー")
        .description(msg)
}

fn resolve_http_error(e: &reqwest::Error) -> &'static str {
    match e.status().map(|s| s.as_u16()) {
        Some(400..=499) => "URLが正しくないか、Modが見つかりませんでした。",
        Some(500..=599) => "サービス側でエラーが発生しました。しばらく後に試してください。",
        _ => "エラーが発生しました。",
    }
}

fn handle_fetch_error(site: &str, slug: &str, e: FetchError) -> CreateEmbed {
    if let Some(re) = e.downcast_ref::<reqwest::Error>() {
        if re.status().is_some_and(|s| s.is_client_error()) {
            warn!("[{site}] {slug}: {e}");
        } else {
            error!("[{site}] {slug}: {e}");
        }
        error_embed(resolve_http_error(re))
    } else {
        error!("[{site}] {slug}: {e}");
        error_embed("エラーが発生しました。")
    }
}

async fn fetch_modrinth(client: &HttpClient, slug: &str) -> Result<ModData, FetchError> {
    let base = "https://api.modrinth.com/v2";

    let (proj_res, members_res) = tokio::try_join!(
        client
            .get(format!("{base}/project/{slug}"))
            .header("User-Agent", USER_AGENT)
            .send(),
        client
            .get(format!("{base}/project/{slug}/members"))
            .header("User-Agent", USER_AGENT)
            .send(),
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
        .take(MAX_AUTHORS)
        .map(|m| {
            (
                m.user.username.clone(),
                format!("https://modrinth.com/user/{}", m.user.username),
            )
        })
        .collect();

    Ok(ModData {
        title: proj.title,
        description: truncate_str(&proj.description.unwrap_or_default(), MAX_DESCRIPTION_CHARS),
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
        .query(&[("gameId", MINECRAFT_GAME_ID), ("slug", slug)]);
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
        .take(MAX_AUTHORS)
        .map(|a| (a.name.clone(), a.url.clone()))
        .collect();

    let fallback_url = format!("https://www.curseforge.com/minecraft/{category}/{slug}");

    Ok(Some(ModData {
        title: mod_data.name,
        description: truncate_str(
            &mod_data.summary.unwrap_or_default(),
            MAX_DESCRIPTION_CHARS,
        ),
        url: mod_data
            .links
            .and_then(|l| l.website_url)
            .unwrap_or(fallback_url),
        icon_url: mod_data
            .logo
            .as_ref()
            .and_then(|l| l.thumbnail_url.as_ref().or(l.url.as_ref()))
            .cloned(),
        downloads: mod_data.download_count as u64,
        game_versions,
        loaders,
        published: mod_data.date_created,
        updated: mod_data.date_modified,
        authors,
    }))
}

pub struct Handler {
    http_client: Arc<HttpClient>,
    reply_map: DashMap<MessageId, (ChannelId, Vec<MessageId>, Instant)>,
    mod_cache: DashMap<String, (Instant, ModData)>,
    suppressed_ids: Arc<DashMap<MessageId, ()>>,
}

impl Handler {
    pub fn new() -> Self {
        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("HTTP client build failed");
        Self {
            http_client: Arc::new(client),
            reply_map: DashMap::new(),
            mod_cache: DashMap::new(),
            suppressed_ids: Arc::new(DashMap::new()),
        }
    }

    fn get_cached(&self, key: &str) -> Option<ModData> {
        let entry = self.mod_cache.get(key)?;
        let (created, data) = entry.value();
        if created.elapsed() < Duration::from_secs(CACHE_TTL_SECS) {
            Some(data.clone())
        } else {
            drop(entry);
            self.mod_cache.remove(key);
            None
        }
    }

    fn set_cache(&self, key: String, data: ModData) {
        self.mod_cache.insert(key, (Instant::now(), data));
    }

    async fn fetch_embed_modrinth(&self, slug: String) -> CreateEmbed {
        let cache_key = format!("modrinth:{slug}");
        if let Some(data) = self.get_cached(&cache_key) {
            return build_embed(&data, "Modrinth");
        }
        match fetch_modrinth(&self.http_client, &slug).await {
            Ok(data) => {
                let embed = build_embed(&data, "Modrinth");
                self.set_cache(cache_key, data);
                embed
            }
            Err(e) => handle_fetch_error("Modrinth", &slug, e),
        }
    }

    async fn fetch_embed_curseforge(
        &self,
        slug: String,
        category: String,
        api_key: Option<String>,
    ) -> CreateEmbed {
        let Some(api_key) = api_key else {
            return error_embed("CurseForge APIキーが設定されていません。");
        };
        let cache_key = format!("curseforge:{slug}");
        if let Some(data) = self.get_cached(&cache_key) {
            return build_embed(&data, "CurseForge");
        }
        match fetch_curseforge(&self.http_client, &api_key, &slug, &category).await {
            Ok(Some(data)) => {
                let embed = build_embed(&data, "CurseForge");
                self.set_cache(cache_key, data);
                embed
            }
            Ok(None) => error_embed("URLが正しくないか、Modが見つかりませんでした。"),
            Err(e) => handle_fetch_error("CurseForge", &slug, e),
        }
    }

    async fn build_embeds(
        &self,
        detected: Vec<DetectedUrl>,
        api_key: Option<String>,
        max_embeds: usize,
    ) -> Vec<CreateEmbed> {
        let mut embeds = Vec::new();
        for url in detected.into_iter().take(max_embeds) {
            let embed = match url {
                DetectedUrl::Modrinth { slug } => self.fetch_embed_modrinth(slug).await,
                DetectedUrl::CurseForge { slug, category } => {
                    self.fetch_embed_curseforge(slug, category, api_key.clone()).await
                }
            };
            embeds.push(embed);
        }
        embeds
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

        if mod_info.mode == "channels" && !mod_info.allowed_channel_ids.contains(&msg.channel_id) {
            return;
        }

        if msg
            .flags
            .is_some_and(|f| f.contains(MessageFlags::SUPPRESS_EMBEDS))
        {
            return;
        }

        let detected = detect_urls(&msg.content);
        if detected.is_empty() {
            return;
        }

        self.reply_map.retain(|_, (_, _, created)| {
            created.elapsed() < Duration::from_secs(REPLY_MAP_TTL_SECS)
        });

        let embeds = self
            .build_embeds(
                detected,
                mod_info.curseforge_api_key.clone(),
                mod_info.max_embeds_per_message,
            )
            .await;
        if embeds.is_empty() {
            return;
        }

        let builder = CreateMessage::new()
            .reference_message(&msg)
            .allowed_mentions(CreateAllowedMentions::new().replied_user(false))
            .embeds(embeds);

        match msg.channel_id.send_message(&ctx.http, builder).await {
            Ok(bot_msg) => {
                self.reply_map
                    .insert(msg.id, (msg.channel_id, vec![bot_msg.id], Instant::now()));

                let channel_id = msg.channel_id;
                let message_id = msg.id;
                let http = ctx.http.clone();
                let suppressed = self.suppressed_ids.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    suppressed.insert(message_id, ());
                    let _ = channel_id
                        .edit_message(
                            &http,
                            message_id,
                            EditMessage::new().suppress_embeds(true),
                        )
                        .await;
                });
            }
            Err(e) => warn!("埋め込み送信失敗: {e}"),
        }
    }

    async fn message_update(
        &self,
        ctx: Context,
        _old: Option<Message>,
        _new: Option<Message>,
        event: MessageUpdateEvent,
    ) {
        if self.suppressed_ids.remove(&event.id).is_some() {
            return;
        }

        let Some(content) = &event.content else {
            return;
        };

        let Some((_, (channel_id, bot_msg_ids, _))) = self.reply_map.remove(&event.id) else {
            return;
        };

        let detected = detect_urls(content);
        if detected.is_empty() {
            for id in &bot_msg_ids {
                let _ = channel_id.delete_message(&ctx.http, *id).await;
            }
            return;
        }

        let config = get_config(&ctx).await;
        let mod_info = &config.mod_info;

        let embeds = self
            .build_embeds(
                detected,
                mod_info.curseforge_api_key.clone(),
                mod_info.max_embeds_per_message,
            )
            .await;
        if embeds.is_empty() {
            for id in &bot_msg_ids {
                let _ = channel_id.delete_message(&ctx.http, *id).await;
            }
            return;
        }

        if let Some(&first_id) = bot_msg_ids.first() {
            let edit = EditMessage::new().embeds(embeds);
            match channel_id.edit_message(&ctx.http, first_id, edit).await {
                Ok(_) => {
                    self.reply_map
                        .insert(event.id, (channel_id, bot_msg_ids, Instant::now()));
                }
                Err(e) => warn!("埋め込み編集失敗: {e}"),
            }
        }
    }

    async fn message_delete(
        &self,
        ctx: Context,
        _channel_id: ChannelId,
        deleted_message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        if let Some((_, (ch, bot_ids, _))) = self.reply_map.remove(&deleted_message_id) {
            for id in bot_ids {
                let _ = ch.delete_message(&ctx.http, id).await;
            }
        }
    }
}
