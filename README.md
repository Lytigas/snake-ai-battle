# Snake Bot Battle

## Game Description

It's two-player snake, except the snakes have infinite length. Whoever crashes last wins. If both players crash at the same time, it's a tie. Also known as Tron light cycles. Players collide with boundary walls.

The game is played on a 32x32 board, indexed and with directions as shown:

```txt
0   1   2   3  ...  31
32  33  34  35 ...  63
                                     U
.           .                        |
.             .                 L ------- R
.               .                    |
                                     D

992 993 994 995...   1023
```

Each player/bot will receive information as if they are the player who starts on the left of the board (red) and will start at position 484 (15\*32+4).

## Protocol

Though the game actually does have "sides," (red and blue) the protocol feeds data to each client so as to be rotationally symmetric. Thus, players can code their bots without worrying about making it position agnostic, if they so desire.

Example code implementing the protocol is provided in the `bots` directory.

### Initiating the Game

Bots will begin by connecting over TCP to `127.0.0.1:4040` and sending a string containing the identifier for their bot, followed by a line feed byte (also known as `\n`, newline, UNIX line ending, etc).

### The Game Loop

Turns of the game begin when the server sends an ASCII-encoded pair of integers to the bot, delimited by a space and followed by a line feed. These represent the position of the player bot and the opposing bot, respectively. Bots must then respond with one of `u`, `d`, `l`, or `r`, indicating the direction they would like to advance this turn. These represent up, down, left, and right respectively and are interpreted according to the coordinate diagram above. Note that one player will perceive their motions as inverted in the visualizer. The direction character will be followed by a line feed, ending one cycle of the game loop.

After both bots have sent their moves, they will be carried out, the win state will be updated if applicable, and the game loop will begin again with the server sending updated positions.

### Ending the Game

When the end of the game is reached, the server, rather than sending positions, will send one of `WIN`, `LOSS`, or `TIE`, followed by a linefeed. The connection will then be closed.

### Limits

From the time the server sends the positions, clients have 200ms to respond with their move. Failure to do so will result in an immediate loss (or tie if both players fail on the same turn).
In some cases the server may fail to accurately track time, accidentally allowing a client to take longer. This is unfortunately unavoidable.
Additionally, the second player to be polled by the server has a slight advantage.
In the final tournament, bots will switch sides to ensure fairness.

### An Example Exchange

`<` Indicates messages sent to the client, `>` indicates those sent from the client to the server.

```txt
> my_super_cool_bot
< 484 539
> u
< 485 538
...
> l
< WIN
```

## Implementing your bot

### Via TCP

Example python code using the TCP protocol directly is given in the `bots` directory.
If you're using python, this will be the easiest route to take.

### Via stdin/stdout

If you want to use another language and don't feel like dealing with TCP IO, you may use the `client-adapter` to communicate via stdout/stdin.
`client-adapter` will connect to the server and forward what it receives on stdin to the server, and write what it receives to stdout.
Thus, you need to connect both stdout and stdin of your bot to the `client-adapter` process. This can be done easily on a UNIX-like shell:

```sh
mkfifo to_client
my_bot < to_client | client-adapter > to_client
```

A more robust wrapper script for this behavior is given in the `bots` directory, along with an example bot in python.

## Running Your Bot

The provided server listens for clients on 127.0.0.1:4040, and runs a web-based visualizer on [127.0.0.1:3030](http://127.0.0.1:3030/).
The server will wait for the two clients to connect before starting, and the first toconnect will become red.

If you use TCP IO, you may run your bot as you would an ordinary program. Otherwise, see wrapper script.

Tip: Check out `--help` on the included binaries. They may or may not have useful options.

## Submitting and Other Ground Rules

You may submit either a x86_64-linux-gnu ELF binary, a well-configured (!) docker image, or source code along with necessary assets and reasonable running instructions. Source code is preferred.

No network requests or outside communications are allowed. Bots must be entirely self-contained.

Please warn if your bot will make use of the filesystem.

I can accommodate special considerations within reason. Things like "compile my program with these arguments," or "use this script to CPU-pin it" are reasonable.

## Tournament

### Format

Each of the N submitted bots will play each other bot M times on each side, for a total of M\*N\*(N-1) games. M will likely be a low number, as I don't expect many probabilistic submissions. A win is worth 2 points, a tie is worth 1 point, and a loss 0 points. Winners are the highest overall scorers.

### Schedule

You may periodically submit your bot for intermediate "practice" tournaments. The results will be public. Additionally, I may decide to redistribute (in obfuscated form) any entered bots for others to develop against.

## License

All work in this repo is licensed as follows.

```txt
Copyright 2020 Josh Hejna

Licensed under the Apache License, Version 2.0 (the "License");
you may not use except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
