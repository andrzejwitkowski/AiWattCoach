use std::sync::LazyLock;

pub const PACKED_STREAM_SEGMENT_GUIDANCE: &str = "Stream segments: ps=power [minW,maxW,durationSec]; cs=cadence [minRPM,maxRPM,durationSec]. Walk left-to-right chronologically; min==max means steady. From 3s power / 5s cadence merged ±5W/±2RPM. Power max<10=coasting; cadence max=0=no pedaling. Compare ps work blocks to bl (minw/maxw or minp/maxp+ftp); recovery valleys expected. cs is supporting only. Recent execution streams in volatile rd.w only; stable h.w is metadata. get_selected_workout uses same triplet format.";

pub const PACKED_CALENDAR_AUTHORITY_GUIDANCE: &str = "Calendar authority: prd=only athlete-declared vacation/rest ranges. pd=AI coach plan after saved summary (authoritative projected days). ud/fe/pw=Intervals planned events. fr=calendar-empty in that section (no pw/sd/w); NOT vacation. pd overrides fr:true on same dates. Never infer vacation from fr:true gaps or sparse history.";

pub const PACKED_HEADER_TABLE_PARSING_LINE: &str = "Packed context arrays (w, lt, rc, fe, prd, av, wr, rs, bl, and nested tables) use header-mapped format: an h array defines column keys, and r contains rows of values in the exact same order. Parse accordingly. def_disc/def_ty hoist constant columns out of rows when present.";

pub const PACKED_TRAINING_CONTEXT_LEGEND: &str = "Packed context v3: header-mapped tables {h,r} for homogeneous lists; top-level objects keep scalar fields. i=intervals status (a=activities,e=events). p=profile (fnm,age,hcm,wkg,ftp,hrm,vo2,ap,meds,notes,acfg; av={h:[wd,a,mdm?],r}). rc=races {h:[d,n,km,pri,id],r; def_disc when uniform}. fe=future Intervals events {h:[id,sd,c,ty?,n?,desc?,dur?,tss?,ifv?,np?],r}. prd=only app vacation/rest {h:[id,sd,ed,n?,nt?],r}. h=history: ws/we window, ac count, ttss, ctl/atl/tsb baseline, ftp/ftpd, t7/t28/if28/ef28 aggregates; lt={h:[d,tss],r} daily load (omit per-row ctl/atl/tsb/t7/t28/days—derive from h baselines+tss); w={h:[d,id,n?,dur?,tss?,np?,ftp?,recap?,bl?],r; def_ty when uniform; bl nested table}. g=generated_at epoch. fx=focus {id?,k}. rd=recent days {d,fr,sick,sickn?; w/pw/sd nested tables}. wr=recaps {h:[d,id,rpe?,recap],r}. ud=upcoming {d,fr; pw/sd tables}. pd=projected plan {d; w table}. rs=race window 14d {h:[d,pri,disc,n,days_out],r}. Field keys: id,sd,d,n,ty,c,desc,dur,tss,ifv,np,ftp,pri,disc,km,fr,sick,sickn,pw,doc,done,bl,minp,maxp,minw,maxw,ps,cs,rpe,recap,rest,rr,swid. wr=authoritative saved summaries for recent window.";

pub const ATHLETE_SUMMARY_CALENDAR_GUARD: &str =
    "Never state future vacation/break unless in prd. Never infer rest from low TSS, fr:true, sparse history, or ud gaps.";

static PACKED_TRAINING_CONTEXT_LEGEND_WITH_GUIDANCE: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{PACKED_TRAINING_CONTEXT_LEGEND} {PACKED_STREAM_SEGMENT_GUIDANCE} {PACKED_CALENDAR_AUTHORITY_GUIDANCE} {PACKED_HEADER_TABLE_PARSING_LINE}"
    )
});

pub fn packed_training_context_legend_with_guidance() -> &'static str {
    PACKED_TRAINING_CONTEXT_LEGEND_WITH_GUIDANCE.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_documents_v3_header_tables() {
        let legend = packed_training_context_legend_with_guidance();
        assert!(legend.contains("v3"));
        assert!(legend.contains("header-mapped"));
        assert!(legend.contains("lt={h:[d,tss]"));
        assert!(legend.contains("def_disc"));
        assert!(legend.contains("derive from h baselines"));
    }

    #[test]
    fn legend_includes_mandatory_parsing_line() {
        let legend = packed_training_context_legend_with_guidance();
        assert!(legend.contains(PACKED_HEADER_TABLE_PARSING_LINE));
    }
}
