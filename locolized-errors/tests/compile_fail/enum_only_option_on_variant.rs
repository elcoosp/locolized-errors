use locolized_errors::LocalizedApiError;

#[derive(Debug, LocalizedApiError)]
#[localized(kind = "bad-request")]
pub enum Example {
    #[localized(kind = "not-found")]
    Variant,
}

fn main() {}
