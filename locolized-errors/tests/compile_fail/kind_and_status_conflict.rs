use locolized_errors::LocalizedApiError;

#[derive(Debug, LocalizedApiError)]
#[localized(kind = "bad-request", status = 418)]
pub enum Example {
    Variant,
}

fn main() {}
