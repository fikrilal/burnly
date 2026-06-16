use iana_time_zone::GetTimezoneError;

pub fn resolve() -> Result<String, GetTimezoneError> {
    iana_time_zone::get_timezone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_non_empty_timezone() {
        assert!(!resolve()
            .expect("resolve system timezone")
            .trim()
            .is_empty());
    }
}
