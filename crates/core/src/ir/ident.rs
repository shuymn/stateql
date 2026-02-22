#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub value: String,
    pub quoted: bool,
}

impl Ident {
    pub fn quoted(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            quoted: true,
        }
    }

    pub fn unquoted(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            quoted: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedName {
    pub schema: Option<Ident>,
    pub name: Ident,
}
