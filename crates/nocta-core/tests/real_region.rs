//! Locates real Roblox datacenter addresses with the same lookup and parsing Nocta uses for the
//! server region filter. Asking Roblox for a server's address needs a signed-in account; this
//! checks the rest of the chain against the live location service.
use nocta_core::{launch::region::parse_location, models::ServerRegion, roblox::web::RobloxWeb};
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "asks a live IP location service"]
async fn real_roblox_datacenters_are_placed_in_their_regions() {
    let web = RobloxWeb::new().unwrap();
    let cancel = CancellationToken::new();
    let mut seen = Vec::new();
    for address in ["128.116.1.1", "128.116.32.1", "128.116.50.1"] {
        let url = reqwest::Url::parse(&format!(
            "{}/{address}?fields=success,continent_code,country_code,city",
            web.endpoints.geo
        ))
        .unwrap();
        let answer = web.get_raw(url, None, None, &cancel).await.unwrap();
        let location = parse_location(&answer.json().unwrap()).expect("a location");
        println!("{address}: {:?} ({})", location.region, location.place);
        seen.push(location.region);
    }
    assert!(seen.contains(&ServerRegion::NorthAmerica));
    assert!(
        seen.contains(&ServerRegion::Asia),
        "Roblox has servers in Singapore"
    );
}
