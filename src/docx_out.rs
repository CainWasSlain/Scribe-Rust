use crate::minutes::Minutes;
use crate::transcribe::Segment;
use anyhow::Result;
use docx_rs::{Docx, Paragraph, Run, Table, TableCell, TableRow};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

pub fn mmss(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// Consecutive same-speaker segments become one paragraph (list of segment indices).
pub fn paragraphs(segs: &[Segment]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        match out.last_mut() {
            Some(p) if segs[p[0]].speaker == s.speaker => p.push(i),
            _ => out.push(vec![i]),
        }
    }
    out
}

fn heading(text: &str, size: usize) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text).bold().size(size))
}

fn plain(text: &str) -> Paragraph {
    Paragraph::new().add_run(Run::new().add_text(text))
}

fn bullet(ts: i64, who: &str, text: &str) -> Paragraph {
    Paragraph::new()
        .add_run(Run::new().add_text(format!("•  [{}] {who}: ", mmss(ts))).bold())
        .add_run(Run::new().add_text(text))
}

pub fn write(path: &Path, title_date: &str, segs: &[Segment], m: &Minutes) -> Result<()> {
    let duration = segs.iter().map(|s| s.end_ms).max().unwrap_or(0);
    let langs: BTreeSet<&str> = segs.iter().map(|s| s.lang.as_str()).collect();
    let key_segs: HashSet<usize> = m.key_points.iter().map(|s| s.seg).collect();

    let mut doc = Docx::new()
        .add_paragraph(heading("Meeting minutes", 40))
        .add_paragraph(plain(&format!(
            "{title_date}   ·   Duration {}   ·   Language: {}",
            mmss(duration),
            langs.into_iter().collect::<Vec<_>>().join(", ")
        )))
        .add_paragraph(heading("Topics", 28))
        .add_paragraph(plain(&m.topics.join(", ")))
        .add_paragraph(heading("Summary", 28));
    for s in &m.summary {
        doc = doc.add_paragraph(plain(&s.text));
    }
    doc = doc.add_paragraph(heading("Key points", 28));
    for s in &m.key_points {
        doc = doc.add_paragraph(bullet(s.start_ms, &s.speaker, &s.text));
    }
    doc = doc.add_paragraph(heading("Decisions", 28));
    if m.decisions.is_empty() {
        doc = doc.add_paragraph(plain("None detected."));
    }
    for s in &m.decisions {
        doc = doc.add_paragraph(bullet(s.start_ms, &s.speaker, &s.text));
    }
    doc = doc.add_paragraph(heading("Action items", 28));
    if m.actions.is_empty() {
        doc = doc.add_paragraph(plain("None detected."));
    } else {
        let cell = |t: &str, bold: bool| {
            let r = Run::new().add_text(t);
            TableCell::new().add_paragraph(Paragraph::new().add_run(if bold { r.bold() } else { r }))
        };
        let mut rows = vec![TableRow::new(vec![cell("Owner", true), cell("Task", true), cell("Time", true)])];
        for (owner, s) in &m.actions {
            rows.push(TableRow::new(vec![cell(owner, false), cell(&s.text, false), cell(&mmss(s.start_ms), false)]));
        }
        doc = doc.add_table(Table::new(rows));
    }

    doc = doc
        .add_paragraph(heading("Full transcript", 28))
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("Highlighted lines are the key points.").italic()));
    for p in paragraphs(segs) {
        let first = &segs[p[0]];
        let mut para = Paragraph::new()
            .add_run(Run::new().add_text(format!("[{}] {}: ", mmss(first.start_ms), first.speaker)).bold());
        for i in p {
            let run = Run::new().add_text(format!("{} ", segs[i].text));
            para = para.add_run(if key_segs.contains(&i) { run.highlight("yellow") } else { run });
        }
        doc = doc.add_paragraph(para);
    }

    doc.build().pack(std::fs::File::create(path)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minutes::{build, seg};

    #[test]
    fn mmss_format() {
        assert_eq!(mmss(0), "00:00");
        assert_eq!(mmss(65_400), "01:05");
        assert_eq!(mmss(3_725_000), "1:02:05");
    }

    #[test]
    fn paragraphs_group_by_speaker() {
        let segs = vec![seg(0, "Me", "a"), seg(1, "Me", "b"), seg(2, "Others", "c"), seg(3, "Me", "d")];
        assert_eq!(paragraphs(&segs), vec![vec![0, 1], vec![2], vec![3]]);
    }

    #[test]
    fn writes_a_docx_zip() {
        let segs = vec![
            seg(0, "Others", "We decided to ship the release on Monday."),
            seg(4_000, "Me", "I'll send the release notes to the release team today."),
            seg(8_000, "Others", "നമസ്കാരം, റിലീസ് തിങ്കളാഴ്ച."),
        ];
        let dir = std::env::temp_dir().join(format!("scribe-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.docx");
        write(&p, "2026-11-08 14:30", &segs, &build(&segs)).unwrap();
        let bytes = std::fs::read(&p).unwrap();
        assert_eq!(&bytes[..2], b"PK"); // .docx is a zip
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
