use std::fmt::Display;

#[derive(Debug, Clone, poise::Modal)]
pub struct BasicQuestionData {
    #[name = "質問のタイトル (わかりやすいように質問内容を要約してください)"]
    #[placeholder = "質問のタイトルを入力してください"]
    #[min_length = 10]
    #[max_length = 100]
    pub title: String,
    #[name = "Minecraftのバージョン"]
    #[placeholder = "Minecraftのバージョンを入力してください"]
    #[min_length = 3]
    #[max_length = 20]
    pub mc_version: String,
    #[name = "Modローダー (Forge, Fabric, NeoForge, Quilt, その他)"]
    #[placeholder = "使用しているModローダーを選択してください"]
    #[min_length = 3]
    #[max_length = 20]
    pub loader: String,
    #[name = "Modローダーのバージョン"]
    #[placeholder = "使用しているModローダーのバージョンを入力してください"]
    #[min_length = 3]
    #[max_length = 20]
    pub loader_version: String,
}

impl Display for BasicQuestionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "### 基本情報\n- Minecraftバージョン: {}\n- Modローダー: {}\n- Modローダーバージョン: {}",
            self.mc_version, self.loader, self.loader_version
        )
    }
}

#[derive(Debug, Clone, poise::Modal)]
pub struct DetailedQuestionData {
    #[name = "質問の内容 (詳細な質問内容を入力してください)"]
    #[placeholder = "質問の内容を入力してください"]
    #[min_length = 20]
    #[max_length = 1000]
    #[paragraph]
    pub content: String,
    #[name = "問題解決の達成基準"]
    #[placeholder = "問題解決の達成基準を入力してください"]
    #[min_length = 20]
    #[max_length = 1000]
    #[paragraph]
    pub content2: String,
    #[name = "試したこと・調べたこと"]
    #[placeholder = "質問を行う前に試したことや調べたことを入力してください"]
    #[min_length = 20]
    #[max_length = 1000]
    #[paragraph]
    pub content3: String,
}

impl Display for DetailedQuestionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "### 質問内容\n- 質問内容:\n{}\n- 問題解決の達成基準:\n{}\n- 試したこと・調べたこと:\n{}",
            self.content, self.content2, self.content3
        )
    }
}

impl Default for DetailedQuestionData {
    fn default() -> Self {
        Self {
            content: "例:クラッシュした, 変な挙動をする, modの扱い方がわからない".to_string(),
            content2: "例: クラッシュから抜け出したい, このような挙動にしたい, このmodでこのようなことがしたい".to_string(),
            content3: "例:○○というサイトに掲載されてた対処法を試した\n推奨:mclo.gs にて変換したクラッシュレポート、lastet.logのリンクを貼る".to_string(),
        }
    }
}
