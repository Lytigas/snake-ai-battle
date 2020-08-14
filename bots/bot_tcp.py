import socket
import io
import sys
import time


class GameIo:
    def __init__(self, socket, name):
        self.socket = socket
        self.socket_file = self.socket.makefile()
        self.read_buffer = bytearray()
        self.name = name
        self.initted = False
        self.gameover = False

    # Do one game loop iteration. Returns a (my_pos, their_pos) tuple, or WIN/LOSS/TIE strings if those situations arise
    # On the first call control_char is ignored; this is the call that starts the game.
    # Calls will likely raise exceptions after a WIN/LOSS/TIE condition, depending on server behavior
    def advance(self, control_char):
        if not self.initted:
            self.socket.send((self.name + "\n").encode("utf-8"))
            self.initted = True
        else:
            self.socket.send((control_char + "\n").encode("utf-8"))
        line = self._wait_for_line()
        if line in ["WIN\n", "LOSS\n", "TIE\n"]:
            self.gameover = True
            return line.strip()
        my_pos, their_pos = (int(i) for i in line.strip().split(" "))
        return my_pos, their_pos

    def is_gameover(self):
        return self.gameover

    def _wait_for_line(self) -> str:
        while True:
            idx = self.read_buffer.find(b"\n")
            if idx == -1:
                new_data = self.socket.recv(32)
                if not new_data:
                    raise EOFError("Socket closed, read 0 bytes")
                self.read_buffer.extend(new_data)
                continue
            line = self.read_buffer[0 : (idx + 1)]
            del self.read_buffer[0 : (idx + 1)]
            return line.decode("utf-8")


# Encapsulates game logic and provides a simple way for subclasses to play the
# game by implementing a single method
class TronBot:
    def __init__(self, name, host=("127.0.0.1", 4040)):
        s = socket.create_connection(host)
        self._game_io = GameIo(s, name)
        self._win_state = None

    # Returns win_state()
    def run(self) -> str:
        # dummy call to send our name
        data = self._game_io.advance("u")
        if data in ["WIN", "LOSS", "TIE"]:
            self._win_state = data
            return self.win_state()
        while True:
            step = self.next_move(data[0], data[1])
            data = self._game_io.advance(step)
            if data in ["WIN", "LOSS", "TIE"]:
                self._win_state = data
                return self.win_state()

    def win_state(self) -> str:
        return self._win_state

    # Override this method! Should return a one-character direction string as outlined in the protocol.
    def next_move(self, my_pos, their_pos) -> str:
        raise NotImplementedError("Override this method")


# Example bot
class SequenceBot(TronBot):
    def __init__(self, name, sequence):
        super().__init__(name)
        self.seq = sequence
        self.counter = -1

    def next_move(self, my_pos, their_pos):
        print(my_pos, their_pos)
        self.counter += 1
        time.sleep(0.05)
        return self.seq[self.counter % len(self.seq)]


seq = ["u", "r"]
bot = SequenceBot("my_seq_bot", seq)
print(bot.run())
