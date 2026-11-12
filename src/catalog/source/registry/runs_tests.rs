//! Tests of `catalog::source::registry::runs`, against servers as the registry's schema
//! writes them.

use serde_json::json;

use super::*;

#[test]
fn an_address_is_preferred_to_a_package_and_streamable_to_events() {
    let server = json!({
        "remotes": [
            {"type": "sse", "url": "https://mcp.example.com/sse"},
            {"type": "streamable-http", "url": "https://mcp.example.com/mcp",
             "headers": [{"name": "Authorization", "description": "Bearer token",
                          "isRequired": true, "isSecret": true, "value": "Bearer {token}"}]}
        ],
        "packages": [{"registryType": "npm", "identifier": "@example/mcp", "version": "1.0.0",
                      "transport": {"type": "stdio"}}]
    });
    let way = reached(&server).unwrap();
    assert_eq!(
        way.runs,
        Runs::Remote {
            url: "https://mcp.example.com/mcp".into(),
            sse: false,
        }
    );
    assert_eq!(
        way.needs,
        [Need {
            name: "Authorization".into(),
            about: "Bearer token".into(),
            required: true,
            secret: true,
            header: true,
            default: String::new(),
        }]
    );
    // Events alone: those; an address left to fill in, or not over HTTPS, is no address.
    let sse = json!({"remotes": [{"type": "sse", "url": "https://x.example.com/sse"}]});
    assert!(matches!(
        reached(&sse).unwrap().runs,
        Runs::Remote { sse: true, .. }
    ));
    let unfilled = json!({"remotes": [
        {"type": "streamable-http", "url": "https://{tenant}.example.com/mcp"},
        {"type": "streamable-http", "url": "http://example.com/mcp"}]});
    assert_eq!(reached(&unfilled), None);
}

#[test]
fn a_package_is_started_by_the_program_of_its_registry_pinned_to_its_version() {
    let started =
        |package: Value| match reached(&json!({"packages": [package]})).map(|server| server.runs) {
            Some(Runs::Command { command, args }) => Some((command, args)),
            Some(Runs::Remote { .. }) | None => None,
        };
    let npm = json!({"registryType": "npm", "identifier": "@modelcontextprotocol/server-everything",
                     "version": "2.1.0", "transport": {"type": "stdio"},
                     "packageArguments": [{"type": "positional", "value": "stdio"},
                                          {"type": "named", "name": "--port", "value": "8080"},
                                          {"type": "named", "name": "--dir", "value": "{dir}"}]});
    let npm_args = [
        "-y",
        "@modelcontextprotocol/server-everything@2.1.0",
        "stdio",
        "--port",
        "8080",
    ];
    assert_eq!(
        started(npm),
        Some(("npx".into(), npm_args.map(String::from).to_vec()))
    );
    let pypi = json!({"registryType": "pypi", "identifier": "mcp-server-time", "version": "0.6.2",
                      "transport": {"type": "stdio"}});
    assert_eq!(
        started(pypi).unwrap(),
        ("uvx".into(), vec!["mcp-server-time==0.6.2".into()])
    );
    let oci = json!({"registryType": "oci", "identifier": "docker.io/mcp/github", "version": "1.2",
                     "transport": {"type": "stdio"},
                     "environmentVariables": [{"name": "GITHUB_TOKEN", "isRequired": true,
                                               "isSecret": true, "description": "A token"},
                                              {"name": "LOG", "default": "info"}]});
    let oci_args = [
        "run",
        "-i",
        "--rm",
        "-e",
        "GITHUB_TOKEN",
        "-e",
        "LOG",
        "docker.io/mcp/github:1.2",
    ];
    assert_eq!(
        started(oci.clone()).unwrap().1,
        oci_args.map(String::from).to_vec()
    );
    let needs = reached(&json!({"packages": [oci]})).unwrap().needs;
    assert_eq!(
        (needs[0].secret, needs[0].header, needs[0].required),
        (true, false, true)
    );
    assert_eq!(needs[1].default, "info");
    // An image named with its tag keeps it; a version said as `latest` pins nothing.
    let tagged = json!({"registryType": "oci", "identifier": "ghcr.io/a/b:2", "version": "2",
                        "transport": {"type": "stdio"}});
    assert_eq!(started(tagged).unwrap().1.last().unwrap(), "ghcr.io/a/b:2");
    let latest = json!({"registryType": "npm", "identifier": "x", "version": "latest",
                        "transport": "stdio"});
    assert_eq!(started(latest).unwrap().1, ["-y", "x"]);
}

#[test]
fn a_package_no_agent_can_start_as_given_is_not_a_way_to_reach_it() {
    for package in [
        json!({"registryType": "nuget", "identifier": "X", "transport": {"type": "stdio"}}),
        json!({"registryType": "npm", "identifier": "x", "transport": {"type": "streamable-http"}}),
        json!({"registryType": "npm", "identifier": "--x", "transport": {"type": "stdio"}}),
        json!({"registryType": "npm", "identifier": "x", "transport": {"type": "stdio"},
               "packageArguments": [{"type": "positional", "valueHint": "path", "isRequired": true}]}),
    ] {
        assert_eq!(reached(&json!({"packages": [package]})), None, "{package}");
    }
    assert_eq!(reached(&json!({})), None);
}
