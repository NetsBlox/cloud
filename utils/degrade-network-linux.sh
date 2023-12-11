echo "Degrading your local network (linux)"
tc qdisc replace dev lo root netem delay 0ms
tc qdisc replace dev lo root netem delay 10ms 10ms 50 distribution normal
tc qdisc replace dev lo root netem loss 50% 25%
tc qdisc replace dev lo root netem corrupt 50%

