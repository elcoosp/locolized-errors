use locolized_errors::LocalizedApiError;

#[derive(Debug, LocalizedApiError)]
pub struct NotAnEnum {
    field: u8,
}

fn main() {}
