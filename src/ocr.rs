use ocrs::{OcrEngine, OcrEngineParams};
use regex::Regex;
use rten::Model;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub struct OcrDetector {
    engine: OcrEngine,
    regex: Regex,
}

impl OcrDetector {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let det_url = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
        let rec_url = "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

        async fn download_if_missing(
            path: &str,
            url: &str,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            if !Path::new(path).exists() {
                println!("Downloading model {}... this might take a moment.", path);
                let response = reqwest::get(url).await?;
                let bytes = response.bytes().await?;
                let mut file = File::create(path)?;
                file.write_all(&bytes)?;
                println!("Downloaded {}.", path);
            }
            Ok(())
        }

        download_if_missing("text-detection.rten", det_url).await?;
        download_if_missing("text-recognition.rten", rec_url).await?;

        let detection_model_data = std::fs::read("text-detection.rten")?;
        let recognition_model_data = std::fs::read("text-recognition.rten")?;

        let detection_model = Model::load(detection_model_data)?;
        let recognition_model = Model::load(recognition_model_data)?;

        // ocrs 0.8 setup
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })?;

        // Regex looking for numbers followed by st, nd, rd, th + space + "race"
        // Made forgiving for spaces inside "2 nd" or trailing artifacts
        let regex = Regex::new(r"(?i)(\d{1,2})\s*(?:st|nd|rd|th)\s*race").unwrap();

        Ok(Self { engine, regex })
    }

    pub fn parse_race_number(&self, width: u32, height: u32, pixels_rgb: &[u8]) -> Option<u32> {
        let img_source = ocrs::ImageSource::from_bytes(pixels_rgb, (width, height)).ok()?;
        let ocr_input = self.engine.prepare_input(img_source).ok()?;
        let word_rects = self.engine.detect_words(&ocr_input).ok()?;
        let line_rects = self.engine.find_text_lines(&ocr_input, &word_rects);
        let text_lines = self.engine.recognize_text(&ocr_input, &line_rects).ok()?;

        for line in text_lines.into_iter().flatten() {
            let lower = line.to_string().to_lowercase();
            
            // Handle the 12th match "Final Race" edge case
            if lower.contains("final") && (lower.contains("race") || lower.contains("rac")) {
                println!("DEBUG OCR matches: {}", lower);
                return Some(12);
            }
            
            if let Some(caps) = self.regex.captures(&lower) {
                if let Ok(num) = caps[1].parse::<u32>() {
                    println!("DEBUG OCR matches: {}", lower);
                    if num >= 1 && num <= 12 {
                        return Some(num);
                    }
                }
            }
        }

        None
    }

    pub fn extract_raw_text(&self, width: u32, height: u32, pixels_rgb: &[u8]) -> Option<String> {
        let img_source = ocrs::ImageSource::from_bytes(pixels_rgb, (width, height)).ok()?;
        let ocr_input = self.engine.prepare_input(img_source).ok()?;
        let word_rects = self.engine.detect_words(&ocr_input).ok()?;
        let line_rects = self.engine.find_text_lines(&ocr_input, &word_rects);
        let text_lines = self.engine.recognize_text(&ocr_input, &line_rects).ok()?;

        let mut extraction = String::new();
        for line in text_lines.into_iter().flatten() {
            extraction.push_str(&line.to_string());
            extraction.push('\n');
        }
        Some(extraction)
    }
}
