use scraper::{Html, Selector};
use std::io::{BufRead, Write};

static MSU_MASTER_URL: &str = "http://master.cmc.msu.ru/";

fn main() {
    if std::env::args().len() != 4 {
        eprintln!("wrong number of arguments: please specify <bookmarkfile> <token> <chat_id>");
        std::process::exit(1);
    }

    let opts = Opts::from_args();
    let news_handler = NewsHandlerImpl::from_opts(opts.clone());

    request_loop(&opts.bookmarkfile, 1000 * 60, news_handler);
}

#[derive(Debug)]
struct News {
    id: String,
    date: String,
    body: String,
}

#[derive(Clone)]
struct Opts {
    bookmarkfile: String,
    token: String,
    chat_id: String,
}

trait NewsHandler {
    fn handle_news(&self, news: &News);
}

struct NewsHandlerImpl {
    char_id: String,
    token: String,
}

impl Opts {
    fn from_args() -> Opts {
        Opts {
            bookmarkfile: std::env::args().nth(0).unwrap(),
            token: std::env::args().nth(1).unwrap(),
            chat_id: std::env::args().nth(2).unwrap(),
        }
    }
}

impl NewsHandlerImpl {
    fn from_opts(opts: Opts) -> NewsHandlerImpl {
        NewsHandlerImpl {
            char_id: opts.chat_id,
            token: opts.token,
        }
    }
}

impl NewsHandler for NewsHandlerImpl {
    fn handle_news(&self, news: &News) {
        let re = regex::Regex::new(r"\s+").unwrap();
        let body = news
            .body
            .replace("<p>", "")
            .replace("</p>", "\n")
            .replace("<br>", "\n");

        let body = re.replace_all(&body, " ");
        println!("{}\n{}\n", body, news.date);
    }
}

fn request_loop(bookmarkfile: &str, interval: u64, news_handler: impl NewsHandler) {
    loop {
        let html = request_html();
        let mut bookmark = std::collections::HashMap::new();
        if let Err(err) = load_bookmark(bookmarkfile, &mut bookmark) {
            eprintln!("{}", err);
        }

        let mut news: Vec<News> = Vec::new();

        match html {
            Ok(html) => {
                let document = Html::parse_document(&html);
                let selector = Selector::parse(r#"[id^='node-']"#).unwrap();
                document.select(&selector).for_each(|div| {
                    let id = div.value().attr("id").unwrap_or("");

                    if !id.is_empty() {
                        let selector_date = Selector::parse(r#"span[class='submitted']"#).unwrap();
                        let selector_body = Selector::parse(r#"div[class='content']"#).unwrap();
                        let inner_html = Html::parse_fragment(&div.html());
                        let submitted_date = match inner_html.select(&selector_date).next() {
                            Some(span) => span.inner_html(),
                            _ => String::new(),
                        };
                        let body = match inner_html.select(&selector_body).next() {
                            Some(div) => div.inner_html(),
                            _ => String::new(),
                        };

                        if submitted_date.is_empty() {
                            eprintln!("submitted date is empty!");
                        }

                        if body.is_empty() {
                            eprintln!("body is empty!");
                        }

                        if !submitted_date.is_empty() && !body.is_empty() {
                            let empty = String::new();
                            let found_id = bookmark.get(&*id).unwrap_or(&empty);

                            if found_id != &submitted_date {
                                news.push(News {
                                    id: id.to_string(),
                                    date: submitted_date.clone(),
                                    body,
                                });
                                bookmark.insert(id.to_string(), submitted_date);
                            }
                        }
                    }
                })
            }
            Err(err) => eprintln!("{}", err),
        }

        if !news.is_empty() {
            news.sort_by_key(|x| x.id.clone());

            for n in news {
                news_handler.handle_news(&n);
            }
        }

        let res = save_bookmark(bookmarkfile, &bookmark);
        if let Err(err) = res {
            eprintln!("{}", err);
        }

        std::thread::sleep(std::time::Duration::from_millis(interval));
    }
}

fn request_html() -> Result<String, String> {
    let resp = reqwest::blocking::get(MSU_MASTER_URL);
    match resp {
        Ok(resp) => {
            let text = resp.text();
            match text {
                Ok(text) => Ok(text),
                Err(err) => Err(format!("{}", err)),
            }
        }

        Err(err) => Err(format!("{}", err)),
    }
}

fn load_bookmark(
    filename: &str,
    bookmark: &mut std::collections::HashMap<String, String>,
) -> std::io::Result<()> {
    if std::path::Path::new(filename).exists() {
        let lines = read_lines(filename)?;

        for line in lines {
            if let Err(err) = line {
                return Err(err);
            }

            let line = line.unwrap();
            let mut split = line.split(' ');
            let id: &str = split.next().unwrap();
            let date = split.collect::<Vec<&str>>().join(" ");
            bookmark.insert(id.to_string(), date);
        }
    }

    Ok(())
}

fn save_bookmark(
    filename: &str,
    hash: &std::collections::HashMap<String, String>,
) -> std::io::Result<()> {
    let mut tmp_filename = String::from(filename);
    tmp_filename.push_str(".tmp");

    #[allow(unused_must_use)]
    {
        std::fs::remove_file(&tmp_filename);
    }

    let data = hash
        .iter()
        .map(|(k, v)| format!("{} {}", k, v))
        .collect::<Vec<String>>()
        .join("\n");

    write_to_file(&tmp_filename, &data)?;
    std::fs::rename(tmp_filename, filename)?;

    Ok(())
}

fn write_to_file(filename: &str, data: &str) -> std::io::Result<()> {
    let mut file = std::fs::File::create(filename)?;
    write!(file, "{}", data)?;
    file.sync_all()?;
    Ok(())
}

fn read_lines<P>(filename: P) -> std::io::Result<std::io::Lines<std::io::BufReader<std::fs::File>>>
where
    P: AsRef<std::path::Path>,
{
    let file = std::fs::File::open(filename)?;
    Ok(std::io::BufReader::new(file).lines())
}
