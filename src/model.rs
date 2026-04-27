#[derive(serde::Deserialize, sqlx::FromRow, Clone)]
pub struct Submission {
    pub date: Option<String>,
    pub name: String,
    pub email: String,
    pub message: String,
}
