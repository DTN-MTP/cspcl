use hardy_bpv7::{bpsec, bundle::ParsedBundle};

pub(crate) fn bundle_label(data: &[u8]) -> String {
    match ParsedBundle::parse(data, bpsec::no_keys) {
        Ok(parsed) => format!("{} -> {}", parsed.bundle.id, parsed.bundle.destination),
        Err(error) => format!("unparsed bundle: {error}"),
    }
}
