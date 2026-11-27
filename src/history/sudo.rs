use super::History;
use rusqlite::params;

impl History {
    pub fn record_sudo(&self, e: &crate::watcher::SudoEvent) -> rusqlite::Result<()> {
        self.0.execute("INSERT INTO sudo_events(ts,pid,tool,args,chain) VALUES(?1,?2,?3,?4,?5)",
            params![e.ts as i64, e.pid as i64, e.tool, e.args, serde_json::to_string(&e.chain).unwrap_or_default()])?;
        Ok(())
    }

    pub fn recent_sudo(&self, limit: usize) -> rusqlite::Result<Vec<crate::watcher::SudoEvent>> {
        let mut st = self.0.prepare("SELECT ts,pid,tool,args,chain FROM sudo_events ORDER BY id DESC LIMIT ?1")?;
        let rows = st.query_map(params![limit as i64], |r| Ok(crate::watcher::SudoEvent {
            ts: r.get::<_, i64>(0)? as u64, pid: r.get::<_, i64>(1)? as u32, tool: r.get(2)?, args: r.get(3)?,
            chain: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
        }))?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::test_util::scratch;

    #[test]
    fn sudo_rows_from_before_hop_detail_load_next_to_new_ones() {
        let dir = scratch("sudo-old-rows");
        let h = History::at(&dir.join("h.db")).unwrap();
        // Written by a build that stored only pid, exe and command, plus a row with no chain and a hostile command.
        for (ts, args, chain) in [
            (1, "sudo ls", r#"[{"pid":5,"exe":"/bin/zsh","command":"-zsh"}]"#),
            (2, "sudo true", "[]"),
        ] {
            h.0.execute("INSERT INTO sudo_events(ts,pid,tool,args,chain) VALUES(?1,9,'/usr/bin/sudo',?2,?3)", params![ts, args, chain]).unwrap();
        }
        let hop = crate::watcher::Hop { pid: 6, exe: "/bin/bash".into(), command: "bash -c \"a<b&c\"".into(), started: Some(7), tty: None, cwd: Some("/tmp".into()), detail: true, ..Default::default() };
        h.record_sudo(&crate::watcher::SudoEvent { ts: 3, pid: 9, tool: "/usr/bin/sudo".into(), args: "sudo \"x\" <&".into(), chain: vec![hop.clone()] }).unwrap();
        let rows = h.recent_sudo(10).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].chain, vec![hop]);
        assert!(rows[1].chain.is_empty());
        assert_eq!((rows[2].chain[0].pid, rows[2].chain[0].detail, rows[2].chain[0].cwd.clone()), (5, false, None));
        let _ = std::fs::remove_dir_all(dir);
    }
}
