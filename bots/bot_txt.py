import time
import sys

# Meant to be used with client-adapter to enable communicating via stdin/stdout.

seq = ["u"] * 3 + ["r"] * 20 + ["d"] * 5 + ["l"] * 5
print("seq_bot_txt")
counter = 0
while True:
    data = input()
    if data in ["WIN", "LOSS", "TIE"]:
        print(data)
        break
    my_pos, their_pos = (int(i) for i in data.strip().split(" "))
    print(seq[counter % len(seq)])
    # This is how you might do print debugging:
    print("Moving", seq[counter % len(seq)], file=sys.stderr)
    counter += 1
    time.sleep(0.05)
