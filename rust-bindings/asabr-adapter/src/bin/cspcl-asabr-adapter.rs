use std::env;

use cspcl_asabr_adapter::{
    RouteRequest, cspcl_route_decision_status_t, cspcl_route_mode_t, query_route,
    set_contact_plan_path,
};

fn parse_list(value: Option<String>) -> Vec<u16> {
    value
        .unwrap_or_default()
        .split(',')
        .filter_map(|entry| entry.trim().parse::<u16>().ok())
        .collect()
}

fn usage(program: &str) -> ! {
    eprintln!(
        "Usage: {program} query --cp <path> --source <id> --dest <ids> [--excluded <ids>] [--priority <n>] [--size <bytes>] [--expiration <t>] [--current-time <t>] [--timeout-ms <ms>]"
    );
    std::process::exit(2);
}

fn print_decision(decision: &cspcl_asabr_adapter::RouteDecision) {
    let status = match decision.decision_status {
        cspcl_route_decision_status_t::CSPCL_ROUTE_DECISION_FOUND => "FOUND",
        cspcl_route_decision_status_t::CSPCL_ROUTE_DECISION_NO_ROUTE => "NO_ROUTE",
        cspcl_route_decision_status_t::CSPCL_ROUTE_DECISION_PROVIDER_ERROR => "PROVIDER_ERROR",
        cspcl_route_decision_status_t::CSPCL_ROUTE_DECISION_TIMEOUT => "TIMEOUT",
    };

    let mode = match decision.mode {
        cspcl_route_mode_t::CSPCL_ROUTE_MODE_NONE => "NONE",
        cspcl_route_mode_t::CSPCL_ROUTE_MODE_UNICAST => "UNICAST",
        cspcl_route_mode_t::CSPCL_ROUTE_MODE_MULTICAST => "MULTICAST",
    };

    println!("STATUS={status}");
    println!("MODE={mode}");
    println!("DIAG={}", decision.diagnostic.to_string_lossy());
    for hop in &decision.next_hops {
        println!(
            "HOP={},{},{}",
            hop.next_hop_node_id, hop.contact_identifier, hop.estimated_arrival_time
        );
    }
}

fn main() {
    let mut args = env::args();
    let program = args
        .next()
        .unwrap_or_else(|| "cspcl-asabr-adapter".to_string());
    let command = args.next().unwrap_or_else(|| usage(&program));
    if command != "query" {
        usage(&program);
    }

    let mut contact_plan_path = None::<String>;
    let mut source_node_id = None::<u16>;
    let mut destination_nodes = None::<String>;
    let mut excluded_nodes = None::<String>;
    let mut bundle_priority = 0i8;
    let mut bundle_size = 1.0f64;
    let mut bundle_expiration = 0.0f64;
    let mut current_time = 0.0f64;
    let mut timeout_ms = 0u32;

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--cp" => contact_plan_path = args.next(),
            "--source" => source_node_id = args.next().and_then(|value| value.parse().ok()),
            "--dest" => destination_nodes = args.next(),
            "--excluded" => excluded_nodes = args.next(),
            "--priority" => {
                bundle_priority = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0)
            }
            "--size" => {
                bundle_size = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(1.0)
            }
            "--expiration" => {
                bundle_expiration = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0.0)
            }
            "--current-time" => {
                current_time = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0.0)
            }
            "--timeout-ms" => {
                timeout_ms = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0)
            }
            _ => usage(&program),
        }
    }

    let contact_plan_path = contact_plan_path
        .unwrap_or_else(|| env::var("CSPCL_ASABR_CONTACT_PLAN_PATH").unwrap_or_default());
    if contact_plan_path.is_empty() {
        eprintln!("missing contact plan path");
        std::process::exit(2);
    }
    if let Err(err) = set_contact_plan_path(Some(contact_plan_path)) {
        eprintln!("failed to configure contact plan path: {}", err as i32);
        std::process::exit(2);
    }

    let source_node_id = source_node_id.unwrap_or_else(|| {
        eprintln!("missing source node id");
        std::process::exit(2);
    });
    let destinations = parse_list(destination_nodes);
    if destinations.is_empty() {
        eprintln!("missing destination list");
        std::process::exit(2);
    }
    let excluded = parse_list(excluded_nodes);

    let request = RouteRequest {
        source_node_id,
        destinations,
        bundle_priority,
        bundle_size,
        bundle_expiration,
        current_time,
        excluded,
        timeout_ms,
    };

    match query_route(&request) {
        Ok(decision) => {
            print_decision(&decision);
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("routing failed: {}", err as i32);
            std::process::exit(1);
        }
    }
}
