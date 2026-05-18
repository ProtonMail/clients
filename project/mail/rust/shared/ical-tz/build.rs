use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use std::{env, fs};

const WORLD_TERRITORY_ID: &str = "001";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=data/windowsZones.xml");

    // ---

    // https://github.com/unicode-org/cldr/blob/394eff4b41857a515706dd0ecc403a506c841bf9/common/supplemental/windowsZones.xml
    let zones = fs::read_to_string("data/windowsZones.xml").unwrap();
    let zones: SupplementalData = quick_xml::de::from_str(&zones).unwrap();

    let all_zones = &zones.windows_zones.map_timezones.map_zone;

    let key_map: HashMap<_, _> = all_zones
        .iter()
        .filter(|zone| zone.territory == WORLD_TERRITORY_ID)
        .map(|zone| (zone.other.as_str(), zone.ty.as_str()))
        .collect();

    let city_map: HashMap<String, &str> = {
        let mut map: HashMap<String, &str> = HashMap::new();

        for zone in all_zones {
            let Some(default_tz) = key_map.get(zone.other.as_str()) else {
                continue;
            };
            let Some(default_tz) = default_tz.split_whitespace().next() else {
                continue;
            };

            for iana in zone.ty.split_whitespace() {
                if iana.starts_with("Etc/") {
                    continue;
                }
                if let Some(city) = iana.rsplit('/').next() {
                    map.insert(city.replace('_', " "), default_tz);
                }
            }
        }

        map
    };

    // ---

    let out = env::var_os("OUT_DIR").unwrap();
    let out = Path::new(&out).join("windows_zones.rs");

    let key_map_str: String = key_map
        .into_iter()
        .map(|(lhs, rhs)| format!(r#"("{lhs}", "{rhs}")"#))
        .fold(String::new(), |mut out, line| {
            _ = writeln!(out, "{line},");
            out
        });

    let city_map_str: String = city_map
        .into_iter()
        .map(|(city, tz)| format!(r#"("{city}", "{tz}")"#))
        .fold(String::new(), |mut out, line| {
            _ = writeln!(out, "{line},");
            out
        });

    fs::write(
        &out,
        format!(
            "
            use std::collections::HashMap;
            use std::sync::LazyLock;

            pub static MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {{
                HashMap::from_iter(vec![{key_map_str}])
            }});

            pub static CITY_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {{
                HashMap::from_iter(vec![{city_map_str}])
            }});
        "
        ),
    )
    .unwrap();
}

#[derive(Clone, Debug, Deserialize)]
struct SupplementalData {
    #[serde(rename = "windowsZones")]
    windows_zones: WindowsZones,
}

#[derive(Clone, Debug, Deserialize)]
struct WindowsZones {
    #[serde(rename = "mapTimezones")]
    map_timezones: MapTimezones,
}

#[derive(Clone, Debug, Deserialize)]
struct MapTimezones {
    #[serde(rename = "mapZone")]
    map_zone: Vec<MapZone>,
}

#[derive(Clone, Debug, Deserialize)]
struct MapZone {
    #[serde(rename = "@other")]
    other: String,

    #[serde(rename = "@territory")]
    territory: String,

    #[serde(rename = "@type")]
    ty: String,
}
