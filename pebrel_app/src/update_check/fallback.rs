//! GitHub's public latest-release redirect is independent of the REST API quota.
//! The redirect proves a version; the official SHA256SUMS asset can then supply
//! verified package metadata. If absent, retain manual download only.

use std::time::Duration;

use ureq::ResponseExt as _;

use super::{LatestRelease, RELEASES_PAGE, release_download_url};
use crate::i18n::{LanguagePreference, Message};

#[cfg(test)]
const TAG_PREFIX: &str = "https://github.com/suzikuo/pebrel-winc/releases/tag/";

pub(super) fn fetch_latest(status: u16) -> Result<LatestRelease, String> {
    let latest_page = format!("{RELEASES_PAGE}/latest");
    log::info!("update-check: API returned HTTP {status}; checking fork latest-release page");
    // Resolve proxy settings for github.com independently of api.github.com.
    let language =
        LanguagePreference::from(pebrel_settings::RuntimeSettings::load().language).resolved();
    let agent =
        crate::update_proxy::agent(&latest_page, Duration::from_secs(10)).map_err(|error| {
            language.format(
                Message::UpdateCheckFallbackFailed,
                &[("status", &status.to_string()), ("error", &error.to_string())],
            )
        })?;
    let uri = redirected_uri(&agent, &latest_page).map_err(|error| {
        language.format(
            Message::UpdateCheckFallbackFailed,
            &[("status", &status.to_string()), ("error", &error.to_string())],
        )
    })?;
    let release = release_from_uri(&uri)
        .ok_or_else(|| language.text(Message::UpdateCheckUnrecognizedRelease).to_owned())?;
    // The official manifest restores verified download metadata during API limits.
    // If it is unavailable, version discovery still works with manual download.
    let url = release_download_url(&release.version, "SHA256SUMS");
    let agent = crate::update_proxy::agent(&url, Duration::from_secs(10));
    Ok(with_checksums(release, &url, agent))
}

fn with_checksums(
    mut release: LatestRelease,
    url: &str,
    agent: Result<ureq::Agent, ureq::Error>,
) -> LatestRelease {
    if let Ok(agent) = agent
        && let Ok(mut response) = agent.get(url).header("User-Agent", "pebrel").call()
        && let Ok(text) = response.body_mut().with_config().limit(64 * 1024).read_to_string()
    {
        release.asset = super::assets::from_checksums(&release.version, &text);
        release.portable_zip_url = super::assets::current_windows_arch().and_then(|architecture| {
            super::assets::portable_from_checksums(&release.version, architecture, &text)
        });
    }
    release
}

fn redirected_uri(agent: &ureq::Agent, url: &str) -> Result<String, ureq::Error> {
    let response = agent
        .get(url)
        .header("User-Agent", "pebrel")
        .header("Accept", "text/html")
        .config()
        .max_redirects(5)
        .build()
        .call()?;
    // The final response URI is supplied by the HTTP client, not by untrusted
    // markup. No HTML body needs to be downloaded or parsed.
    Ok(response.get_uri().to_string())
}

fn release_from_uri(uri: &str) -> Option<LatestRelease> {
    let tag = uri.strip_prefix(RELEASES_PAGE)?.strip_prefix("/tag/")?;
    // Official stable releases use major.minor.patch. Reject login pages,
    // arbitrary tags, extra path/query/fragment components and preview tags.
    let version = super::stable_version_from_tag(tag)?;
    Some(LatestRelease { version: version.to_owned(), asset: None, portable_zip_url: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_proxy::test_support::{Server, response};

    #[test]
    fn unavailable_optional_manifest_keeps_the_discovered_version_for_manual_download() {
        let discovered = || release_from_uri(&format!("{TAG_PREFIX}winc-v1.9.0")).unwrap();
        let release = with_checksums(
            discovered(),
            "http://checksums.update.invalid/SHA256SUMS",
            Err(ureq::Error::InvalidProxyUrl),
        );
        assert_eq!(release.version, "1.9.0");
        assert!(release.asset.is_none());

        let server = Server::start(vec![response("503 Service Unavailable", "", "unavailable")]);
        let release = with_checksums(
            discovered(),
            "http://checksums.update.invalid/SHA256SUMS",
            Ok(server.network_agent("http", None, &[])),
        );
        assert_eq!(release.version, "1.9.0");
        assert!(release.asset.is_none());
        assert_eq!(server.finish().len(), 1);
    }

    #[test]
    fn rate_limited_api_falls_back_and_discovers_an_upgrade_without_install_authority() {
        for status in ["403 Forbidden", "429 Too Many Requests"] {
            let server = Server::start(vec![response(status, "", "API rate limit exceeded")]);
            let result = super::super::fetch_release_with_fallback(
                &server.agent(&[]),
                "http://api.update.invalid/latest",
                |code| {
                    assert!(matches!(code, 403 | 429));
                    Ok(release_from_uri(&format!("{TAG_PREFIX}winc-v1.9.0")).unwrap())
                },
            )
            .unwrap();
            assert!(super::super::is_newer(&result.version, "1.8.2"));
            assert!(!super::super::is_newer(&result.version, "1.9.0"));
            assert!(result.asset.is_none(), "a version redirect grants no install authority");
            assert_eq!(server.finish().len(), 1);
        }
    }

    #[test]
    fn successful_api_and_other_errors_never_call_the_fallback() {
        for (status, body, succeeds) in [
            ("200 OK", r#"{"tag_name":"winc-v1.9.0","assets":[]}"#, true),
            ("200 OK", "not JSON", false),
            ("404 Not Found", "missing release", false),
            ("500 Internal Server Error", "server error", false),
        ] {
            let server = Server::start(vec![response(status, "", body)]);
            let result = super::super::fetch_release_with_fallback(
                &server.agent(&[]),
                "http://api.update.invalid/latest",
                |_| panic!("fallback is only for access/rate limiting"),
            );
            assert_eq!(result.is_ok(), succeeds);
            server.finish();
        }
    }

    #[test]
    fn failed_fallback_stays_an_error_instead_of_claiming_up_to_date() {
        let server = Server::start(vec![response("403 Forbidden", "", "rate limited")]);
        let result = super::super::fetch_release_with_fallback(
            &server.agent(&[]),
            "http://api.update.invalid/latest",
            |_| Err("release page unavailable".into()),
        );
        assert_eq!(result.unwrap_err(), "release page unavailable");
        server.finish();
    }

    #[test]
    fn redirect_transport_uses_the_final_location_without_parsing_html() {
        // The local CONNECT fixture uses HTTP; production agent requires HTTPS.
        let destination = "http://github.com/suzikuo/pebrel-winc/releases/tag/winc-v1.9.0";
        let server = Server::start(vec![
            response("302 Found", &format!("Location: {destination}\r\n"), ""),
            response("200 OK", "Content-Type: text/html\r\n", "<html>no metadata</html>"),
        ]);
        let uri =
            redirected_uri(&server.agent(&[]), "http://github.com/suzikuo/pebrel-winc/releases/latest")
                .unwrap();
        assert_eq!(uri, destination);
        let requests = server.finish();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].1.contains("/releases/latest"));
        assert!(requests[1].1.contains("/releases/tag/winc-v1.9.0"));
    }

    #[test]
    fn only_exact_fork_stable_tag_locations_are_accepted() {
        assert_eq!(
            release_from_uri(&format!("{TAG_PREFIX}winc-v1.9.0")).unwrap().version,
            "1.9.0"
        );
        for uri in [
            "https://github.com/suzikuo/pebrel-winc/releases/tag/v1.9.0",
            "https://github.com/Kuddev/pebrel/releases/tag/winc-v1.9.0",
            "https://github.com/suzikuo/pebrel-winc/releases/latest",
            "https://github.com/login",
            "https://github.com/Other/pebrel/releases/tag/winc-v99.0.0",
            "http://github.com/suzikuo/pebrel-winc/releases/tag/winc-v1.9.0",
            "https://github.com.evil.invalid/suzikuo/pebrel-winc/releases/tag/winc-v1.9.0",
            "https://github.com/suzikuo/pebrel-winc/releases/tag/winc-v1.9.0?foo=bar",
            "https://github.com/suzikuo/pebrel-winc/releases/tag/winc-v1.9.0#fragment",
            "https://github.com/suzikuo/pebrel-winc/releases/tag/winc-v1.9.0/extra",
            "https://github.com/suzikuo/pebrel-winc/releases/tag/winc-v1.9.0-rc1",
            "https://github.com/suzikuo/pebrel-winc/releases/tag/relay-abcd",
            "https://github.com/suzikuo/pebrel-winc/releases/tag/winc-v1..0",
            "https://github.com/suzikuo/pebrel-winc/releases/tag/winc-v01.9.0",
            "https://github.com/suzikuo/pebrel-winc/releases/tag/winc-v18446744073709551616.0.0",
        ] {
            assert!(release_from_uri(uri).is_none(), "{uri}");
        }
    }

    #[test]
    #[ignore = "contacts public GitHub; run explicitly for native network acceptance"]
    fn live_update_check_survives_api_rate_limiting() {
        let release = super::super::fetch_latest_release().expect("live public update metadata");
        let fallback = fetch_latest(403).expect("live official release fallback");
        assert_eq!(release.version, fallback.version);
    }
}
