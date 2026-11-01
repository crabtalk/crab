use berm_lang::Schema;

#[derive(Debug, Clone, Copy, Schema)]
pub enum Format {
    Markdown,
    Html,
    Links,
}
