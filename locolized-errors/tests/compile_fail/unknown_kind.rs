use locolized_errors::LocalizedApiError;

#[derive(Debug, LocalizedApiError)]
pub enum TeapotError {
    Brewed,
}

fn main() {}
