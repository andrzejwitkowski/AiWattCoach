pub(crate) fn non_empty_context_parts<'a>(
    parts: [(&'static str, &'a str); 3],
) -> Vec<(&'static str, &'a str)> {
    parts
        .into_iter()
        .filter(|(_, content)| !content.trim().is_empty())
        .collect()
}

pub(crate) const PACKED_TRAINING_CONTEXT_LEGEND: &str = "Packed context legend: v=schema version, i=intervals status, p=athlete profile, h=historical training, g=generated_at epoch seconds, fx=focus, rd=recent days, ud=upcoming days, pd=projected days. Common inner fields: id=identifier, k=kind, d=date, sd=start_date_local, n=name, ty=activity type, c=category, desc=description, fr=free day, sick=sick day flag, sickn=sick note, w=workouts, pw=planned workouts, doc=raw workout doc, done=completed flag, dur=duration seconds, tss=training stress score, ifv=intensity_factor, ef=efficiency_factor, np=normalized_power_watts, ftp=ftp_watts, vi=variability_index, rpe=session rpe, bl=interval blocks, minp/maxp=target percent FTP bounds, minw/maxw=target watt bounds, c5=cadence values in 5-second buckets. Intervals status uses a=activities status and e=events status. Profile fields include fnm=full name, hcm=height cm, wkg=weight kg, hrm=max heart rate, vo2=VO2 max, ap=athlete prompt, acfg=availability configured, av=weekly availability, wd=weekday, a=available, mdm=max duration minutes. In training_context_volatile recent workout power may be packed in pc as an array of level:seconds strings. For pc, each raw watts sample is first rounded to the nearest 10W bucket, then each level is round((watts / ftp)^2.5 * 100). Consecutive samples that land on the same encoded level are run-length encoded as level:seconds. Only an isolated 1-second spike or dip is smoothed when both neighboring levels match, the change is under 3 levels, and neither the surrounding level nor the changed level is in the FTP zone 90-110.";
