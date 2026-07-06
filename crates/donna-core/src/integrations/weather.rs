//! Weather integration — Open-Meteo free API (no API key required).

use serde::Deserialize;
use crate::error::Result;

#[derive(Debug, Deserialize)]
struct OpenMeteoResponse {
    current: CurrentWeather,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature_2m: f64,
    apparent_temperature: f64,
    weather_code: u32,
    wind_speed_10m: f64,
}

pub struct WeatherSummary {
    pub temp_c: f64,
    pub feels_like_c: f64,
    pub description: &'static str,
    pub wind_kmh: f64,
}

fn wmo_description(code: u32) -> &'static str {
    match code {
        0 => "Clear sky",
        1..=3 => "Partly cloudy",
        45 | 48 => "Foggy",
        51..=55 => "Drizzle",
        61..=65 => "Rain",
        71..=75 => "Snow",
        80..=82 => "Rain showers",
        95 => "Thunderstorm",
        _ => "Mixed conditions",
    }
}

/// Fetch current weather for a lat/lon (Open-Meteo, no key needed).
pub async fn fetch(lat: f64, lon: f64) -> Result<WeatherSummary> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
         &current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m\
         &wind_speed_unit=kmh"
    );
    let resp: OpenMeteoResponse = reqwest::get(&url).await?.json().await?;
    Ok(WeatherSummary {
        temp_c: resp.current.temperature_2m,
        feels_like_c: resp.current.apparent_temperature,
        description: wmo_description(resp.current.weather_code),
        wind_kmh: resp.current.wind_speed_10m,
    })
}

pub fn format_summary(w: &WeatherSummary) -> String {
    format!(
        "{}°C (feels {}°C), {}, wind {} km/h",
        w.temp_c.round() as i32,
        w.feels_like_c.round() as i32,
        w.description,
        w.wind_kmh.round() as u32,
    )
}
