use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tera::{Context, Tera};

#[derive(Debug, Deserialize)]
struct BugResponse {
    id: i32,
    cf_rank: Option<String>,
    alias: Option<String>,
    summary: String,
    blocks: Vec<i32>,
}

#[derive(Debug, Deserialize)]
struct Response {
    bugs: Vec<BugResponse>,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize)]
struct BugId(i32);

#[derive(Debug)]
struct Bug {
    summary: String,
    blocks: Vec<BugId>,
    rank: i32,
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

#[derive(Serialize)]
struct ProjectInfo {
    id: i32,
    severity: i32,
    url: String,
    summary: String,
    bug_count: usize,
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
                    rank: bug.cf_rank.map_or(-1, |r| r.parse::<i32>().unwrap()),
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
    let url = "https://bugzilla.mozilla.org/rest/bug?product=Core&component=Graphics: WebRender&include_fields=blocks,alias,summary,id,cf_rank&resolution=---&limit=0";

    let tera = Tera::new("templates/*.html").unwrap();

    let response: Response = reqwest::blocking::get(url).unwrap().json().unwrap();
    let bugs = BugList::new(response.bugs);

    let mut project_count = 0;
    let mut project_bug_info = HashMap::new();

    for (id, bug) in &bugs.bugs {
        if bug.summary.contains("[project]") {
            let bug_url = format!("https://bugzilla.mozilla.org/show_bug.cgi?id={}", id.0);
            let summary = bug.summary.strip_prefix("[meta] [project] ").unwrap();

            project_bug_info.insert(*id, ProjectInfo {
                id: id.0,
                url: bug_url,
                summary: summary.to_string(),
                bug_count: 0,
                severity: bug.rank,
            });

            project_count += 1;
        }
    }

    let mut unreachable_count = 0;
    let mut unreachable_bug_info = Vec::new();

    fn block_project_bugs(
        id: BugId,
        bug_list: &BugList,
        project_bug_info: &mut HashMap<BugId, ProjectInfo>,
    ) {
        if let Some(bug) = bug_list.bugs.get(&id) {
            for blocker_id in &bug.blocks {
                if let Some(project) = project_bug_info.get_mut(blocker_id) {
                    project.bug_count += 1;
                }

                block_project_bugs(
                    *blocker_id,
                    bug_list,
                    project_bug_info,
                );
            }
        }
    }

    for (id, bug) in &bugs.bugs {
        let bug_url = format!("https://bugzilla.mozilla.org/show_bug.cgi?id={}", id.0);

        if bugs.blocks_wr_projects(id) {
            block_project_bugs(
                *id,
                &bugs,
                &mut project_bug_info,
            );
        } else {
            unreachable_bug_info.push(BugInfo {
                id: id.0,
                url: bug_url.clone(),
                summary: bug.summary.clone(),
            });

            unreachable_count += 1;
        }


    }

    let mut ctx = Context::new();
    ctx.insert("bugs", &unreachable_bug_info);
    let result = tera.render("template.html", &ctx).unwrap();
    std::fs::write("bugs.html", result).unwrap();
    println!("Found {} unreachable bugs", unreachable_count);

    let mut project_bug_list = Vec::new();
    let mut bugs_in_projects = 0;
    for (_, project) in project_bug_info {
        bugs_in_projects += project.bug_count;
        project_bug_list.push(project);
    }
    project_bug_list.sort_by_key(|p| p.severity);

    let mut ctx = Context::new();
    ctx.insert("projects", &project_bug_list);
    let result = tera.render("summary.html", &ctx).unwrap();
    std::fs::write("projects.html", result).unwrap();

    println!("Found {} projects", project_count);
    println!("Found {} bugs attached to projects", bugs_in_projects);
}
