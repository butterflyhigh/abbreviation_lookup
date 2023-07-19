#![allow(dead_code)]

mod py;

use scraper::{Html, Selector};
use reqwest;
use regex::Regex;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use csv;
use std::{thread::sleep, time::Duration, sync::Mutex, sync::Arc, str::FromStr};
use tokio::task::JoinHandle;
use futures::future::join_all;
use rand::seq::*;

const URL: &str = "https://www.acronymfinder.com";
const RANDOM_URL: &str = "https://www.acronymfinder.com/random.aspx";
const RE: &str = r#" \(.*\)"#;
const EXAMPLE_SELECTOR: &str = "div.samples-list > article > div.samples-list__item__content";
const TITLE_SELECTOR: &str = "title";

pub async fn search_acronym(search: String, category: Category) -> Result<Vec<String>, reqwest::Error> {
    let mut res = Vec::<String>::new();
    let table_selector = Selector::parse("div.tab-content > table > tbody").unwrap();
    let res_selector = Selector::parse("td.result-list__body__meaning").unwrap();

    let full_url = match category {
        Category::All => format!("{URL}/{search}.html"),
        Category::IT => format!("{URL}/Information-Technology/{search}.html"),
        Category::Science => format!("{URL}/Science-and-Medicine/{search}.html"),
        Category::Gov => format!("{URL}/Military-and-Government/{search}.html"),
        Category::Org => format!("{URL}/Organizations/{search}.html"),
        Category::Business => format!("{URL}/Business/{search}.html"),
        Category::Slang => format!("{URL}/Slang/{search}.html")
    };

    let doc = reqwest::get(full_url).await?.text().await?;
    let html = Html::parse_document(&doc);

    for table in html.select(&table_selector) {
        for item in table.select(&res_selector) {
            res.push(item.text().last().unwrap().into());
        }
    }

    Ok(res)
}

fn parse(doc: String, re: &Regex, example_selector: &Selector, title_selector: &Selector) -> Data {
    let html = Html::parse_document(&doc);

    let full_title = html
        .select(title_selector)
        .last()
        .unwrap()
        .text()
        .collect::<String>();

    let split: Vec<&str> = full_title.split(" - ").collect();
    let acronym = split[0].to_string();
    let full_def = split[1].replace(" | AcronymFinder", "");
    let def = re.replace(&full_def, "").to_string();

    let first = html.select(example_selector).take(1).collect::<Vec<_>>()[0];
    let text = re.replace(&first.text().collect::<String>(), "").to_string();
    let data = Data { text: text, abbr: acronym.clone(), definition: def.clone() };

    data
}

pub async fn generate_training_data(num_samples: usize, output_path: String) -> std::io::Result<()> {
    let mut writer = csv::Writer::from_path(output_path).unwrap();
    let bar = ProgressBar::new(num_samples as u64);
    let example_selector = Selector::parse(EXAMPLE_SELECTOR).unwrap();
    let title_selector = Selector::parse(TITLE_SELECTOR).unwrap();
    let re = Regex::new(RE).unwrap();

    for _ in 0..num_samples {
        match reqwest::get(RANDOM_URL).await {
            Ok(page) => {
                match page.text().await {
                    Ok(doc) => {
                        writer.serialize(parse(doc, &re, &example_selector, &title_selector)).unwrap();
                    }

                    Err(e) => { println!("WARNNG: {e}"); }
                }
            }

            Err(e) => { println!("WARNING: {e}"); }
        }
        bar.inc(1);
    }
    bar.finish_with_message("Done!");

    writer.flush().unwrap();
    Ok(())
}

pub async fn generate_training_data_async(num_samples: usize, output_path: String) -> std::io::Result<()> {
    let mut writer = csv::Writer::from_path(output_path).unwrap();
    let example_selector = Selector::parse(EXAMPLE_SELECTOR).unwrap();
    let title_selector = Selector::parse(TITLE_SELECTOR).unwrap();
    let re = Regex::new(RE).unwrap();

    let mut handles = Vec::<JoinHandle<Result<Data, reqwest::Error>>>::new();

    for _ in 0..num_samples {
        let re = re.clone();
        let title_selector = title_selector.clone();
        let example_selector = example_selector.clone();

        handles.push(tokio::spawn(async move {
            let page_data = match reqwest::get(RANDOM_URL).await {
                Ok(page) => {
                    match page.text().await {
                        Ok(doc) => {
                            Ok(parse(doc, &re, &example_selector, &title_selector))
                        }
    
                        Err(e) => { println!("WARNNG: {e}"); Err(e) }
                    }
                }
    
                Err(e) => { println!("WARNING: {e}"); Err(e) }
            };

            page_data
        }));

        sleep(Duration::from_millis(250));
    }

    let data_bumpy = join_all(handles).await; // bumpy because its not flat, so smart

    for item in data_bumpy {
        match item {
            Ok(val) => {
                match val {
                    Ok(row) => {
                        writer.serialize(row).unwrap();
                    }
                    Err(_) => { }
                }
            }
            Err(_) => { }
        }
    }

    Ok(())
}

pub async fn format_data_for_mlm(data: Vec<Data>, num_answers: usize, output_path: String) {
    let mut writer = csv::Writer::from_path(output_path).unwrap();
    let mut handles = Vec::<JoinHandle<Result<MaskedData, reqwest::Error>>>::new();
    let tested = Arc::new(Mutex::new(Vec::<String>::new()));

    writer.write_record(["text", "label", "1", "2", "3", "4", "5", "6", "7", "8"]).unwrap();

    for item in data {
        let tested = tested.clone();

        if !tested.lock().unwrap().contains(&item.abbr) {
            handles.push(tokio::spawn(async move {
                let binding = item.definition.to_lowercase();
                let definition = binding.trim().to_string();
                let abbr = item.abbr;
                let masked_text = item.text.to_lowercase().replace(&definition, &format!("[{}]", abbr));

                match reqwest::get(format!("{URL}/{abbr}.html")).await {
                    Ok(res) => {
                        match res.text().await {
                            Ok(_) => {
                                let mut all_answers: Vec<String> = search_acronym(abbr.clone(), Category::All)
                                    .await
                                    .unwrap()
                                    .iter()
                                    .map(|ans| ans.to_lowercase())
                                    .collect();

                                {
                                    let mut lock = tested.lock().unwrap();
                                    lock.push(abbr.clone());
                                }

                                // TODO: add a list of known too-small acronyms to not do again
                                if all_answers.len() < num_answers {
                                    panic!("Only {} answers, skipping abbreviation {abbr}", all_answers.len());
                                }

                                all_answers.truncate(num_answers);

                                if !all_answers.contains(&definition.to_lowercase()) {
                                    all_answers.pop();
                                    all_answers.push(definition.to_lowercase());
                                }

                                all_answers.shuffle(&mut rand::thread_rng());

                                let mut idx = 99;
                                for i in 0..all_answers.len() {
                                    if all_answers[i].eq_ignore_ascii_case(&definition) {
                                        idx = i;
                                    }
                                }

                                let data = MaskedData {
                                    text: masked_text,
                                    answers: all_answers,
                                    correct_answer_idx: idx
                                };

                                Ok(data)
                            }

                            Err(e) => {
                                println!("WARNING: {e}");
                                Err(e)
                            }
                        }
                    }

                    Err(e) => {
                        println!("WARNING: {e}");
                        Err(e)
                    }
                }
            }))
        };

        sleep(Duration::from_millis(100));
    }

    let all_data = join_all(handles).await;

    for item in all_data {
        match item {
            Ok(val) => {
                match val {
                    Ok(data) => {
                        let mut ser = vec![data.text, format!("{}", data.correct_answer_idx)];
                        ser.extend(data.answers);
                        writer.write_record(ser).unwrap();
                    }
                    Err(_) => {}
                }
            }
            Err(_) => {}
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Data {
    text: String,
    abbr: String,
    definition: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MaskedData {
    pub text: String,
    pub answers: Vec<String>,
    pub correct_answer_idx: usize
}

pub enum Category {
    All,
    IT,
    Science,
    Gov,
    Org,
    Business,
    Slang
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseCategoryError;

impl FromStr for Category {
    type Err = ParseCategoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("All") {
            return Ok(Category::All);
        } else if s.eq_ignore_ascii_case("IT") {
            return Ok(Category::IT);
        } else if s.eq_ignore_ascii_case("Science") {
            return Ok(Category::Science);
        } else if s.eq_ignore_ascii_case("Gov") {
            return Ok(Category::Gov);
        } else if s.eq_ignore_ascii_case("Org") {
            return Ok(Category::Org);
        } else if s.eq_ignore_ascii_case("Business") {
            return Ok(Category::Business);
        } else if s.eq_ignore_ascii_case("SLang") {
            return Ok(Category::Slang);
        }

        Err(ParseCategoryError)
    }
}