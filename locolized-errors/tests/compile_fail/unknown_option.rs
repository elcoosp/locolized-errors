use locolized_errors::LocalizedApiError;

#[derive(Debug, LocalizedApiError)]
#[localized(kind = "bad-request", bogus = 1)]
pub enum Example {
    Variant,
}

fn main() {}
