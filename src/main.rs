#![deny(unused_must_use)]

use anyhow::Context;
use std::collections::BTreeMap;

use std::time::Duration;
// use selenium_rs::webdriver::{Browser, WebDriver};
use thirtyfour::prelude::*;

#[derive(Debug)]
struct QuizGroup {
    quizs: Vec<Quiz>,
    submit_btn: WebElement,
}

#[derive(Debug)]
struct Quiz {
    question: String,
    handle: QuizHandle,
}

#[derive(Debug)]
struct QuizHandle {
    elements: WebElement,
    choice_true: WebElement,
    choice_false: WebElement,
}

impl QuizHandle {
    pub async fn check(&self, val: bool) -> WebDriverResult<()> {
        let checkbox_ele = if val {
            &self.choice_true
        } else {
            &self.choice_false
        };
        checkbox_ele.click().await
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://localhost:9515", caps).await?;

    // Go to quiz page
    goto_quiz(&driver).await?;
    println!("Arrive at quiz page");

    // Extract quiz information and element into quiz group
    let quiz_group = make_quiz_group(&driver).await?;

    // Initially submit with all quiz answer set to false
    for quiz in &quiz_group.quizs {
        quiz.handle.check(false).await?;
    }
    quiz_group.submit_btn.click().await?;
    let ModalInfo {
        score: mut best_score,
        max_score,
        close_btn,
    } = extract_modal(&driver).await?;
    close_btn.click().await?;

    // Solving
    let mut qa = BTreeMap::new(); // store question and solved answer for later display
    for quiz in quiz_group.quizs {
        if best_score >= max_score {
            break;
        }

        // try setting this quiz from false to true and submit it
        quiz.handle.check(true).await?;
        quiz_group.submit_btn.click().await?;
        let ModalInfo {
            score,
            max_score: _,
            close_btn,
        } = extract_modal(&driver).await?;
        close_btn.click().await?;

        if score < best_score {
            // if the new score is worst, reset quiz to false
            quiz.handle.check(false).await?;
            println!("|{}|❌|", quiz.question);
            qa.insert(quiz.question, false);
        } else {
            // if the new score is better, use it as best score
            best_score = score;
            println!("|{}|✔️|", quiz.question);
            qa.insert(quiz.question, true);
        }
    }
    println!("===============");

    // Display q&a, will be in alphabetic order
    println!("|Q|A|\n|-|-|");
    for (q, a) in qa.iter() {
        println!("|{}|{}|", q, if *a {'✔'} else {'❌'});
    }

    // Always explicitly close the browser.
    // driver.quit().await?;

    Ok(())
}

struct ModalInfo {
    score: u32,
    max_score: u32,
    close_btn: WebElement,
}

async fn extract_modal(driver: &WebDriver) -> anyhow::Result<ModalInfo> {
    let score: u32 = driver
        .query(By::Css("[data-part=score-obtained]"))
        .wait(Duration::from_secs(3), Duration::from_millis(500))
        .first()
        .await?
        .text()
        .await?
        .parse()
        .context("parsing score-obtained")?;
    let max_score: u32 = driver
        .find(By::Css("[data-part=score-total]"))
        .await?
        .text()
        .await?
        .trim_start_matches('/')
        .parse()
        .context("parsing score-total")?;
    let close_btn = driver
        .find(By::Id("cvocp-quiz-modal-quizresult-closemodal"))
        .await?;
    Ok(ModalInfo {
        score,
        max_score,
        close_btn,
    })
}

async fn make_quiz_group(driver: &WebDriver) -> anyhow::Result<QuizGroup> {
    // Extract quiz element
    let quizes_ele = driver.find(By::Id("cvocp-quiz-body")).await?;
    let quiz_eles = quizes_ele
        .find_all(By::ClassName("cvocp-quiz-item"))
        .await?;

    // Extract information and element, create quiz group struct
    let mut quizs = Vec::new();
    for quiz_ele in quiz_eles {
        let question = quiz_ele.find(By::Css("span")).await?.text().await?;

        let choices_ele = quiz_ele.find(By::Css("[data-part=choices]")).await?;
        let mut choices = choices_ele
            .find_all(By::Css("[data-part=checkbox-img]"))
            .await?;
        assert_eq!(choices.len(), 2);
        let choice_true = choices.swap_remove(0);
        let choice_false = choices.swap_remove(0);

        let handle = QuizHandle {
            elements: quiz_ele,
            choice_true,
            choice_false,
        };
        quizs.push(Quiz { question, handle });
    }
    let quiz_group = QuizGroup {
        quizs,
        submit_btn: driver.find(By::Id("cvocp-quiz-submit-button")).await?,
    };
    Ok(quiz_group)
}

async fn goto_quiz(driver: &WebDriver) -> anyhow::Result<()> {
    // Navigate to mcv login screen.
    driver.goto("https://www.mycourseville.com").await?;

    // Poll until until login
    println!("Waiting for login...");
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let url = driver.current_url().await?;
        let is_login = url.query().map_or(false, |q| q.starts_with("code="));
        if is_login {
            break;
        }
    }
    println!("login detected");

    // Navigate to SE Review Questions
    driver
        .goto("https://www.mycourseville.com/?q=onlinecourse/course/31053")
        .await?;

    println!("Wait to click first quiz link");
    let quiz_link_ele = driver
        .query(By::ClassName("cvposter-lp-item-si-member-panel"))
        .wait(Duration::from_secs(3), Duration::from_millis(500))
        .first()
        .await?
        .find(By::Css("a"))
        .await?;
    let link = quiz_link_ele
        .attr("href")
        .await?
        .context("expect SE Review question to have link")?;
    driver.goto(link).await?;

    Ok(())
}
