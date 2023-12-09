echo "Degrading your local network (linux)"
tc qdisc replace dev lo root netem delay 50ms
tc qdisc replace dev lo root netem delay 50ms 400ms 50 distribution normal
tc qdisc replace dev lo root netem loss 50% 25%
tc qdisc replace dev lo root netem corrupt 50%

