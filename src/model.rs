#[derive(serde::Deserialize, sqlx::FromRow, Clone)]
pub struct Submission {
    pub date: Option<chrono::NaiveDateTime>,
    pub name: String,
    pub email: String,
    pub message: String,
}
