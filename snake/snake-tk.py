import copy
import random
import tkinter as tk
from tkinter import ttk
from enum import Enum

BACKGROUND = '#444444'
LINE_COLOR = '#ffffff'
BODY_COLOR = '#eeeeee'
HEAD_COLOR = '#eeeeee'
APPLE_COLOR = '#ffa0a0'
UPDATE_INTERVAL = 500

class Direction(Enum):
    North = 0
    South = 1
    West = 2
    East = 3

    @staticmethod
    def reversed(self):
        if self == Direction.North:
            return Direction.South
        elif self == Direction.South:
            return Direction.North
        elif self == Direction.East:
            return Direction.West
        elif self == Direction.West:
            return Direction.East

class Node:
    def __init__(self, x: int, y: int, color, id = None):
        self.x = x
        self.y = y
        self.color = color
        self.id = id

    def walk(self, direction):
        if direction == Direction.North:
            self.y -= 1
        elif direction == Direction.South:
            self.y += 1
        elif direction == Direction.West:
            self.x -= 1
        elif direction == Direction.East:
            self.x += 1

class Snake:
    def __init__(self,
        initial_x: int,
        initial_y: int,
        length: int,
        initial_direction = Direction.North,
    ):
        if length <= 0:
            raise ValueError('Invalid length')

        self.nodes = [Node(initial_x, initial_y, HEAD_COLOR)]
        self.nodes[-1].color = HEAD_COLOR
        self.direction = initial_direction
        self.previous_direction = initial_direction
        self.hidden_length = length - 1

    def walk(self):
        self.previous_direction = self.direction

        head = copy.deepcopy(self.head)
        self.nodes[-1].color = BODY_COLOR
        head.walk(self.direction)

        if self.hidden_length > 0:
            self.hidden_length -= 1
            discard_node = None
        else:
            discard_node = self.nodes.pop(0)
        self.nodes.append(head)

        return (head, discard_node)

    def turn(self, direction):
        if direction == Direction.reversed(self.previous_direction):
            return
        else:
            self.direction = direction

    @property
    def head(self):
        return self.nodes[-1]

class ItemType(Enum):
    Empty = 0
    Snake = 1
    Apple = 2
    Out = 3

class GameExecutor:
    def __init__(self, x: int, y: int):
        self.x = x
        self.y = y
        self.snakes = [Snake(int(x / 2), int(y / 2), 2)]
        self.apples = []
        self.new_nodes = [self.snakes[0].nodes[0]]
        self.discard_nodes = []

    def search(self, x: int, y: int):
        if x < 0 or y < 0 or x >= self.x or y >= self.y:
            return ItemType.Out

        for apple in self.apples:
            if apple.x == x and apple.y == y:
                return ItemType.Apple

        for snake in self.snakes:
            for node in snake.nodes:
                if node.x == x and node.y == y:
                    return ItemType.Snake

        return ItemType.Empty

    def delete_apple(self, x: int, y: int):
        for i in range(len(self.apples)):
            apple = self.apples[i]
            if apple.x == x and apple.y == y:
                self.discard_nodes.append(self.apples.pop(i))

    def new_apple(self, n: int = 1):
        i = 0
        while i < n:
            gen_x = random.randint(0, self.x)
            gen_y = random.randint(0, self.y)
            if self.search(gen_x, gen_y) != ItemType.Empty:
                continue
            else:
                node = Node(gen_x, gen_y, APPLE_COLOR)
                self.apples.append(node)
                self.new_nodes.append(node)
            i += 1

    def walk(self):
        dead_snakes = []
        for i in range(len(self.snakes)):
            snake = self.snakes[i]
            head = copy.deepcopy(snake.head)
            head.walk(snake.direction)
            target = self.search(head.x, head.y)

            if target == ItemType.Apple:
                self.delete_apple(head.x, head.y)
                snake.hidden_length += 1
                nn, dn = snake.walk()
                self.new_nodes.append(nn)
                if dn is not None:
                    self.discard_nodes.append(dn)
            elif target == ItemType.Empty:
                nn, dn = snake.walk()
                self.new_nodes.append(nn)
                if dn is not None:
                    self.discard_nodes.append(dn)
            elif target == ItemType.Out or target == ItemType.Snake:
                dead_snakes.append(i)
                self.discard_nodes += snake.nodes

        if len(self.apples) == 0:
            self.new_apple()

        for i in reversed(dead_snakes):
            del(self.snakes[i])

class GameWidget(tk.Canvas):
    def __init__(self, parent, x: int, y: int):
        super().__init__(parent, background=BACKGROUND)
        self.executor = GameExecutor(x, y)

    def update(self):
        x = self.executor.x
        y = self.executor.y

        w = int(self['width'])
        h = int(self['height'])
        bw = w / x
        bh = h / y

        self.executor.walk()
        for new_node in self.executor.new_nodes:
            new_node.id = self.create_rectangle(new_node.x * bw, new_node.y * bh, (new_node.x + 1) * bw, (new_node.y + 1) * bh, fill=new_node.color)
        self.executor.new_nodes = []

        for discard_node in self.executor.discard_nodes:
            self.delete(discard_node.id)
        self.executor.discard_nodes = []

    def create_map(self, x: int = None, y: int = None):
        self.executor.new_nodes = []
        def adder(snake):
            self.executor.new_nodes += snake.nodes
        list(map(adder, self.executor.snakes))
        self.executor.new_nodes += self.executor.apples

        x = self.executor.x if x is None else x
        y = self.executor.y if y is None else y

        w = int(self['width'])
        h = int(self['height'])
        bw = w / x
        bh = h / y

        for ncol in range(0, x + 1):
            line_x = int(bw * ncol)
            id = self.create_line(line_x, 0, line_x, h - 1, fill=LINE_COLOR)
            self.tag_lower(id)

        for nrow in range(0, y + 1):
            line_y = int(bh * nrow)
            id = self.create_line(0, line_y, w - 1, line_y, fill=LINE_COLOR)
            self.tag_lower(id)

    def clear(self):
        self.delete(tk.ALL)

class SnakeGame(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title('Snake')
        self.wm_title('Snake')
        self.geometry('500x500')

        self.w = 0
        self.h = 0

        # init widgets
        self.game_frame = tk.Frame(self, bg=BACKGROUND, highlightthickness=0)
        self.game_widget = GameWidget(self.game_frame, 20, 20)

        # register resize callback
        self.bind('<Configure>', self._resize_callback)

        # register updater
        def update():
            nonlocal self
            self.game_widget.update()
            self.after(UPDATE_INTERVAL, update)

        update()

        # register control keys
        def factory(snake_id, direction):
            nonlocal self
            def callback(event):
                nonlocal self, direction, snake_id
                executor = self.game_widget.executor
                if len(executor.snakes) >= snake_id:
                    executor.snakes[snake_id].turn(direction)
            return callback
        self.bind('<Up>', factory(0, Direction.North))
        self.bind('<Down>', factory(0, Direction.South))
        self.bind('<Left>', factory(0, Direction.West))
        self.bind('<Right>', factory(0, Direction.East))

    def place_widgets(self):
        w = self.w
        h = self.h
        self.game_frame.place(x=0, y=0, width=w, height=h, anchor=tk.NW)
        self.game_widget.place(x=0, y=0, width=w, height=h, anchor=tk.NW)
        self.game_widget['width'] = w
        self.game_widget['height'] = h

    def _resize_callback(self, event):
        if event is not None:
            if self.winfo_width() != self.w or self.winfo_height() != self.h:
                self.w = self.winfo_width()
                self.h = self.winfo_height()
                self.place_widgets()
                self.game_widget.clear()
                self.game_widget.create_map()

if __name__ == '__main__':
    game = SnakeGame()
    game.mainloop()
