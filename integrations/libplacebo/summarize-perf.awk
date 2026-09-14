# Summarize all three rotated runs; never select the best observation.
function median(k, m, a,b,c,t) {
    a=value[k,m,1]; b=value[k,m,2]; c=value[k,m,3];
    if (a>b) {t=a;a=b;b=t} if (b>c) {t=b;b=c;c=t} if (a>b) b=a;
    return b;
}
/^PERF / {
    delete field;
    for (i=2;i<=NF;i++) {split($i,p,"=");field[p[1]]=p[2]}
    k=field["input"] " " field["mode"] " " field["engine"];
    run=++count[k];
    for (m in field) value[k,m,run]=field[m]+0;
}
END {
    if (!length(count)) exit 1;
    print "input mode engine fps_median fps_min fps_max submit_wall_ms submit_cpu_ms collect_wall_ms latency_p50_ms latency_p95_ms setup_ms warm_ms process_cpu_ms peak_rss_kib";
    for (k in count) {
        if (count[k]!=3) {print "expected three runs for " k > "/dev/stderr";exit 1}
        lo=hi=value[k,"fps",1];
        for (i=2;i<=3;i++) {v=value[k,"fps",i]; if(v<lo)lo=v;if(v>hi)hi=v}
        printf "%s %.3f %.3f %.3f",k,median(k,"fps"),lo,hi;
        n=split("submit_wall_ms submit_cpu_ms collect_wall_ms latency_p50_ms latency_p95_ms setup_ms warm_ms process_cpu_ms peak_rss_kib",metrics," ");
        for(i=1;i<=n;i++)printf " %.6f",median(k,metrics[i]);
        printf "\n";
    }
}
