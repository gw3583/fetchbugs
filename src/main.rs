use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tera::{Context, Tera};

#[derive(Debug, Deserialize)]
struct BugResponse {
    id: i32,
    alias: Option<String>,
    summary: String,
    blocks: Vec<i32>,
}

#[derive(Debug, Deserialize)]
struct Response {
    bugs: Vec<BugResponse>,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct BugId(i32);

#[derive(Debug)]
struct Bug {
    summary: String,
    blocks: Vec<BugId>,
}

struct BugList {
    bugs: HashMap<BugId, Bug>,
    root_project_id: BugId,
}

#[derive(Serialize)]
struct BugInfo {
    id: i32,
    url: String,
    summary: String,
}

impl BugList {
    fn new(bug_list: Vec<BugResponse>) -> Self {
        let mut bugs = HashMap::new();
        let mut root_project_id = None;

        for bug in bug_list {
            let id = BugId(bug.id);

            if let Some("wr-projects") = bug.alias.as_ref().map(|s| s.as_str()) {
                assert!(root_project_id.is_none());
                root_project_id = Some(id);
            }

            bugs.insert(
                id,
                Bug {
                    summary: bug.summary,
                    blocks: bug.blocks.iter().map(|id| BugId(*id)).collect(),
                }
            );
        }

        BugList {
            bugs,
            root_project_id: root_project_id.unwrap(),
        }
    }

    fn blocks_wr_projects(&self, id: &BugId) -> bool {
        if *id == self.root_project_id {
            return true;
        }

        match self.bugs.get(id) {
            Some(bug) => {
                for id in &bug.blocks {
                    if self.blocks_wr_projects(id) {
                        return true;
                    }
                }
            }
            None => {
                // Could be referencing a sec bug, or a bug outside the gfx::wr component
            }
        }

        false
    }
}

fn main() {
    let url = "https://bugzilla.mozilla.org/rest/bug?product=Core&component=Graphics: WebRender&include_fields=blocks,alias,summary,id&resolution=---&limit=0";

    let tera = Tera::new("templates/*.html").unwrap();
    let mut ctx = Context::new();

    let response: Response = reqwest::blocking::get(url).unwrap().json().unwrap();
    let bugs = BugList::new(response.bugs);

    let mut count = 0;
    let mut bug_info = Vec::new();

    for (id, bug) in &bugs.bugs {
        if !bugs.blocks_wr_projects(id) {
            let bug_url = format!("https://bugzilla.mozilla.org/show_bug.cgi?id={}", id.0);

            bug_info.push(BugInfo {
                id: id.0,
                url: bug_url,
                summary: bug.summary.clone(),
            });

            count += 1;
        }
    }

    ctx.insert("bugs", &bug_info);
    let result = tera.render("template.html", &ctx).unwrap();
    std::fs::write("bugs.html", result).unwrap();

    println!("Found {} unreachable bugs", count);
}
