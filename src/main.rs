use anyhow::{Context, Result};
use clap::Parser;
use hdf5::File;
use image::GrayImage;
use indicatif::ProgressBar;
use reqwest::blocking::get;
use serde::Deserialize;

const METERS_PER_LAT_DEGREE: f64 = 111000.;
const BATCH_SIZE: usize = 50;

#[derive(Debug, Deserialize)]
struct ElevationResponse {
    elevations: Vec<f64>,
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Extract elevation maps from IGN API")]
struct Args {
    /// Latitude of the map center
    #[arg(required(true))]
    latitude: f64,

    /// Longitude of the map center
    #[arg(required(true))]
    longitude: f64,

    /// Size of the map in meters
    #[arg(short, long, default_value = "1000.")]
    size: f64,

    /// Resolution of the map in meters
    #[arg(short, long, default_value = "50.")]
    resolution: f64,

    /// Path of the output
    #[arg(short, long, default_value = "heights.dat")]
    output: String,

    /// Path of the image
    #[arg(long, default_value = None)]
    image: Option<String>,
}

// Calculate the x and y positions of the map points as lattitude/longitude.
fn calculate_xy_positions(
    latitude: f64,
    longitude: f64,
    size: f64,
    resolution: f64,
) -> (Vec<f64>, Vec<f64>) {
    let meters_per_lon_degree: f64 =
        METERS_PER_LAT_DEGREE * (2. * std::f64::consts::PI * latitude / 360.).cos();
    let mut x = Vec::<f64>::new();
    let mut y = Vec::<f64>::new();
    let map_size = f64::floor(size / resolution) as i64;
    for i in 0..map_size {
        x.push(longitude - (0.5 * size - (i as f64) * resolution) / meters_per_lon_degree);
        y.push(latitude - (0.5 * size - (i as f64) * resolution) / METERS_PER_LAT_DEGREE);
    }
    (x, y)
}

fn fetch_elevation_from_ign(lon_str: &str, lat_str: &str) -> Result<ElevationResponse> {
    let start_url = "https://wxs.ign.fr/calcul/alti/rest/elevation.json?";
    let end_url = "&zonly=true";
    let full_url = format!("{}{}&{}&{}", start_url, lon_str, lat_str, end_url);

    let response = get(&full_url).context("Failed to get the request")?;

    if response.status().is_success() {
        let elevation_data: ElevationResponse = response
            .json()
            .context("Failed to parse response as JSON")?;
        Ok(elevation_data)
    } else {
        Err(anyhow::anyhow!(
            "Request failed with status: {}",
            response.status()
        ))
    }
}

fn save_elevation_data(
    output: &str,
    heights: &[f64],
    positions: &[(f64, f64)],
    resolution: f64,
) -> Result<()> {
    let file = File::create(output).context("Failed to create HDF5 file")?;
    let dataset = file
        .new_dataset::<f64>()
        .shape([heights.len()])
        .create("heights")
        .context("Failed to create 'heights' dataset")?;
    let _ = dataset.write(&heights);

    let dataset = file
        .new_dataset::<(f64, f64)>()
        .shape([positions.len()])
        .create("positions")
        .context("Failed to create 'positions' dataset")?;
    let _ = dataset.write(&positions);

    let dataset = file
        .new_dataset::<f64>()
        .create("resolution")
        .context("Failed to create 'resolution' scalar")?;
    let _ = dataset.write_scalar(&resolution);

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("Calculating the positions ...");
    let xy = calculate_xy_positions(args.latitude, args.longitude, args.size, args.resolution);
    let map_size = xy.0.len();

    let positions: Vec<(f64, f64)> =
        xy.0.iter()
            .flat_map(|x| xy.1.iter().map(|y| (*x, *y)))
            .collect();

    // Loop over chunks of 'positions' and request/push their elevation in 'heights'
    println!("Fetching the data from the IGN API ...");
    let pb = ProgressBar::new((positions.len() / BATCH_SIZE) as u64);
    let mut heights: Vec<f64> = Vec::with_capacity(positions.len());
    for batch_pos in positions.chunks(BATCH_SIZE) {
        let mut lon_str = "lon=".to_string();
        let mut lat_str = "lat=".to_string();
        for elem in batch_pos {
            lon_str = format!("{}{}|", lon_str, elem.0);
            lat_str = format!("{}{}|", lat_str, elem.1);
        }
        if lon_str.ends_with('|') {
            lon_str.pop(); // Remove the last character
        }
        if lat_str.ends_with('|') {
            lat_str.pop(); // Remove the last character
        }

        match fetch_elevation_from_ign(&lon_str, &lat_str) {
            Ok(elevation_data) => heights.extend(elevation_data.elevations),
            Err(err) => eprintln!("Error fetching elevation data: {}", err),
        }
        pb.inc(1);
    }

    println!("Saving the data to {}", args.output);
    save_elevation_data(&args.output, &heights, &positions, args.resolution)?;

    if let Some(path_image) = args.image {
        // Calculate the min and max values for normalisation and create a u8 Gray image.
        let min: f64 = heights.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max: f64 = heights.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let norm_heights: Vec<u8> = heights
            .iter()
            .map(|h| (f64::powf(2., 8.) * (h - min) / (max - min)) as u8)
            .collect();
        let image = GrayImage::from_vec(map_size as u32, map_size as u32, norm_heights).unwrap();
        let rotated_image = image::imageops::rotate270(&image);
        let _ = rotated_image
            .save(path_image)
            .context("Failed to save the image")?;
    }

    Ok(())
}
