import curses
import argparse
import random
import traceback
import time
import threading
import unicodedata
import sys
import copy
from typing import Tuple

#import numpy as np

__author__ = 'xuanyeovo'
__version__ = '1.0.0'

_p = argparse.ArgumentParser()
_p.add_argument('--level', dest='level', type=int, default=3)
_args = _p.parse_args()

UI_LOCK = False

class _SudokuStopGame(Exception):
    def __init__(self, reason:str):
        super().__init__(f'Game stopped: {reason}')

class _SudokuBackToTitle(Exception):
    def __init__(self):
        super().__init__('')
    
class _SudokuLocked(Exception):
    def __init__(self):
        super().__init__('')

class _SudokuStopRendering(Exception):
    pass

class SudokuGrid:
    def __init__(self, level:int):
        self._level = level
        
        self.METASET = {n for n in range(1, level**2+1)}
        self.METAGRID = [[0 for _ in range(level**2)] for _ in range(level**2)]#np.zeros((level**2, level**2), int)
        self._metagrid = copy.copy(self.METAGRID)
        
        while True:
            try:
                self.shuffle()
            except IndexError:
                continue
            else:
                break
    
    def check_overlay(self):
        overlay = self.get_overlay()
        for y in range(self._level**2):
            s = self.METASET.copy()
            for x in range(self._level**2):
                v = overlay[x][y]
                if v == 0:
                    return False
                else:
                    try:
                        s.remove(v)
                    except:
                        return False
        for x in range(self._level**2):
            s = self.METASET.copy()
            for y in range(self._level**2):
                v = overlay[x][y]
                # 前面已排除空格子的干扰，此处无需再判断
                try:
                    s.remove(v)
                except:
                    return False
        for gridx in range(self._level):
            for gridy in range(self._level):
                s = self.METASET.copy()
                for x in range(self._level):
                    for y in range(self._level):
                        v = overlay[gridx*self._level + x][gridy*self._level + y]
                        try:
                            s.remove(v)
                        except:
                            return False
        return True
        
    def shuffle(self):
        
        def dbg(e=None):
            pass
        def dbg_(e=None):
            print(f'x: {x}, y: {y}')
            print(self.get_grid())
            tuple(map(print, (col_selections, row_selections, subgrid_selections)))
            traceback.print_exception(e)
            input()
            
        level = self.get_level()
        col_selections = [self.METASET.copy() for _ in range(level**2)]
        row_selections = [self.METASET.copy() for _ in range(level**2)]
        subgrid_selections = [self.METASET.copy() for _ in range(level**2)]
        def randselect(posx, posy):
            subgridpos = divmod(posx, level)[0] + divmod(posy, level)[0] * level
            
            selections = col_selections[posx] & \
                row_selections[posy] & \
                subgrid_selections[subgridpos]
            
            try:
                final = random.choice(tuple(selections))
            except IndexError as e:
                dbg(e)
                raise e
            for t in (col_selections[posx], row_selections[posy], subgrid_selections[subgridpos]):
                t.remove(final)
            
            return final
            
        for x in range(0, level**2):
            for y in range(0, level**2):
                dbg()
                self.fill(x, y, randselect(x, y), force=True)
        
    def generate_overlay(self, emptys:float):
        empty_positions = []
        for _ in range( int(emptys * (self._level**4)) ): # 生成留空的坐标
            while True:
                pos = (random.randint(0, self._level**2-1), random.randint(0, self._level**2-1))
                if pos in empty_positions:
                    continue
                else:
                    empty_positions.append(pos)
                    break
        
        self._nonlock = empty_positions
        
        self._overlay = copy.deepcopy(self.METAGRID)
        for x in range(self._level**2):
            for y in range(self._level**2):
                if (x, y) in empty_positions:
                    self._overlay[x][y] = 0
                else:
                    self._overlay[x][y] = self._metagrid[x][y]
        
    def fill(self, x:int, y:int, val:int, force=False) -> int:
        if force:
            self._metagrid[x][y] = val
    
    def get_level(self) -> int:
        return self._level
        
    def get_metagrid(self):# -> np.ndarray:
        return self._metagrid
    
    def get_overlay(self):# -> np.ndarray:
        return self._overlay
        
    def is_nonlock(self, pos:Tuple[int, int]) -> bool:
        return pos in self._nonlock

class SudokuRenderer:
    def __init__(self, config_clear=True):
        global UI_LOCK
        if UI_LOCK:
            raise Exception('There has already been a SudokuRenderer still running')
        else:
            UI_LOCK = True
        
        self._actived = False
        
        self._task = None
        self._runlock = threading.Lock()
        self.init()
        
        self.configs = {
            'secret': False
        }
        
    def __del__(self):
        self.destroy()
    
    def init(self):
        self.destroy()
        self._actived = True
        self._scr = curses.initscr()
        self._win = curses.newwin(*tuple( map( lambda x:x-1, self._scr.getmaxyx() ) ), 0, 0)
        self._win.clear()
        curses.cbreak()
        self._scr.keypad(True)
        curses.noecho()
        curses.curs_set(0)
        curses.start_color()
        curses.nonl()
        curses.use_default_colors()
        curses.init_pair(1, curses.COLOR_WHITE, curses.COLOR_BLACK)
        curses.init_pair(2, curses.COLOR_BLACK, curses.COLOR_WHITE)
        curses.init_pair(3, curses.COLOR_GREEN, curses.COLOR_BLACK)
        curses.init_pair(4, curses.COLOR_YELLOW, curses.COLOR_BLACK)
        curses.init_pair(5, curses.COLOR_RED, curses.COLOR_BLACK)
        curses.init_pair(6, curses.COLOR_RED, curses.COLOR_WHITE)
        
    def async_render(self):
        t = threading.Thread(target=self.render)
        t.start()
        return t
        
    def render(self):
        if not self._actived:
            raise Exception('Please call SudokuRenderer.set_instance first')
        
        self._runlock.acquire()
        self._rendering = True
        
        try:
            self._render_titlescr()
        except _SudokuStopRendering as err:
            pass
        except _SudokuStopGame as err:
            self._runlock.release()
            raise err
        except Exception as err:
            self._runlock.release()
            raise err
        finally:
            self._scr.refresh()
            
        self._runlock.release()
    
    def stop_render(self):
        self._rendering = False
        self._runlock.acquire()
        self._runlock.release()
        
    def _getch(self):
        k = self._scr.getch()
        if not self._rendering:
            raise _SudokuStopRendering('Code caused abort')
        else:
            return k
        
    @staticmethod
    def _get_string_width(i:str):
        result = 0
        for c in i:
            if unicodedata.east_asian_width(c) in ('W', 'F'):
                result += 2
            else:
                result += 1
        return result
    
    # 帮助页面
    def _render_help(self):
        self._scr.clear()
        self._scr.addstr(2, 2, '一个Python实现的数独')
        self._scr.addstr(3, 2, '游戏玩法：在空格中填入合适的数字，使得横竖九宫格')
        self._scr.addstr(4, 2, '内的数字不重复，直到空格全部填满即为胜利')
        self._scr.addstr(6, 2, '按任意键继续...')
        self._scr.getch()
    
    def _get_center_pos(self, s:str, k:float):
        scr_height, scr_width = self._scr.getmaxyx()
        x, y = int(scr_width / 2) - int(self._get_string_width(s) / 2), int(scr_height*k)
        if x < 0 or y >= scr_height:
            x = 0
            y = 0
        return y, x
    
    def _draw_version(self):
        self._scr.addstr(self._scr.getmaxyx()[0]-2, 1, 'Ver 1.0.0')
            
    # 标题界面
    def _render_titlescr(self):
        title = 'PySudoku 数独'
        
        current = 0
        choices = [
            ('新游戏', 0.4, self._render_newgame),
            ('设置', 0.6, self._render_settings),
            ('退出', 0.8, 0)
        ]
        while True:
            i = 0
            self._scr.clear()
            self._scr.addstr(*self._get_center_pos(title, 0.1), title)
            for label, k, fn in choices:
                # 选中项目时的颜色
                color = curses.color_pair(2 if current == i else 1)
                    
                # 确定坐标
                y, x = self._get_center_pos(label, k)
                
                # 画边框和内容
                l = self._get_string_width(label)
                self._scr.addch(y-1, x-1, '╔', color)
                self._scr.addstr(y-1, x, '═'*l, color)
                self._scr.addch(y-1, x+l, '╗', color)
                self._scr.addch(y, x-1, '║', color)
                self._scr.addch(y, x+l, '║', color)
                self._scr.addch(y+1, x-1, '╚', color)
                self._scr.addstr(y+1, x, '═'*l, color)
                self._scr.addch(y+1, x+l, '╝', color)
                self._scr.addstr(y, x, label, color)
                i += 1
                
                # 绘制版本
                self._draw_version()
            
            self._scr.refresh()
            
            # 按键处理
            k = self._getch()
            if k == ord('q'):
                raise _SudokuStopGame('Normally exit')
            elif k == curses.KEY_DOWN and current < len(choices) - 1:
                # 下箭头
                current += 1
            elif k == curses.KEY_UP and current > 0:
                # 上箭头
                current -= 1
            elif k == curses.KEY_ENTER or k == 10 or k == 13:
                # 回车键
                if choices[current][2] == 0:
                    raise _SudokuStopGame('Normally exit')
                else:
                    try:
                        choices[current][2]()
                    except _SudokuBackToTitle:
                        continue
            elif k == ord('?'):
                self._render_help()
        
        raise _SudokuStopGame('Normally exit')
    
    def _render_newgame(self):
        title = 'PySudoku 数独'
        hardness = 1
        selected = 0
        
        choices = (
            ('确定(o)', self._render_game, ord('o')),
            ('返回(q)', 0, ord('q'))
        )
        k = 0
        while True:
            scr_height, scr_width = self._scr.getmaxyx()
            
            self._scr.erase()
            self._scr.addstr(*self._get_center_pos(title, 0.1), title)
            
            # 绘制难度选择器
            color = curses.color_pair(2)
            y, x = self._get_center_pos(' '*11, 0.4)
            l = 11
            self._scr.addch(y-1, x-1, '┌', color)
            self._scr.addstr(y-1, x, '-'*l, color)
            self._scr.addch(y-1, x+l, '┐', color)
            self._scr.addch(y, x-1, '|', color)
            self._scr.addch(y, x+l, '|', color)
            self._scr.addch(y+1, x-1, '└', color)
            self._scr.addstr(y+1, x, '-'*l, color)
            self._scr.addch(y+1, x+l, '┘', color)
            self._scr.addch(y, x+l-1, str(hardness), color)
            self._scr.addstr(y, x, '难度', color)
            for c in range(hardness):
                self._scr.addch(y, x+4+c, '*', curses.color_pair( 3 + int(divmod(c, 2)[0]) ))
                #self._scr.addch(y, x+c, '*')
            
            # 绘制下方选项
            y = int(scr_height * 0.8)
            xs = scr_width / len(choices)
            i = 0
            for label, _, _ in choices:
                color = curses.color_pair(2 if i == selected else 1)
                x = int(i * (scr_width / 2) + (xs / 2 - self._get_string_width(label) / 2))
                self._scr.addstr(y, x, label, color)
                i += 1
            
            # 版本号
            self._draw_version()
            
            # debug
            #self._scr.addnstr(scr_height - 1, 0, f'SELECTED={selected}, LATEST_KEY={k}, FUNCTION={self.__class__.__name__}.{choices[selected][1].__name__ if choices[selected][1] != 0 else "exit"}', scr_width)
            
            self._scr.refresh()
            
            k = self._getch()
            if k == curses.KEY_UP and hardness < 6:
                hardness += 1
            elif k == curses.KEY_DOWN and hardness > 1:
                hardness -= 1
            elif k == curses.KEY_LEFT:
                if selected > 0:
                    selected -= 1
                else:
                    selected = len(choices)-1
            elif k == curses.KEY_RIGHT:
                if selected < len(choices) - 1:
                    selected += 1
                else:
                    selected = 0
            elif k == curses.KEY_ENTER or k == 10 or k == 13:
                self._hardness = hardness
                if choices[selected][1] == 0:
                    raise _SudokuBackToTitle('User exited')
                else:
                    choices[selected][1]()
            else:
                for _, fn, shortcut in choices:
                    if shortcut == k:
                        if fn == 0:
                            raise _SudokuBackToTitle()
                        self._hardness = hardness
                        ret = fn()
                        break
        
    def _render_game(self):
        title = ''
        level = 3
        gamegrid = SudokuGrid(level)
        gamegrid.generate_overlay(self._hardness / 10)
        grid = gamegrid.get_metagrid()
        overlay = gamegrid.get_overlay()
        beg_time = time.time()
        steps = 0
        
        input_mode = 0
        
        #i = 0
        #for s in ('=========DEBUG MESSAGE BEGIN=========', 'META GRID INFORMATION', *tuple(map(str, grid.get_overlay())), '==========DEBUG MESSAGE END==========', '按任意键继续...'):
        #    self._scr.addstr(i, 0, s)
        #    i += 1
        
        #width = level**2 + level + 1 + (level - 1) * level
        width = 2 * (level**2) + 1
        height = level**2 + level + 1
        
        tmp = ['═'*(2*level-1) for _ in range(level)]
        tmp2 = [' '*(2*level-1) for _ in range(level)]
        
        cursor_pos = [0, 0]
        
        while True:
            scr_y, scr_x = self._scr.getmaxyx()
            beg_y, beg_x = 5, int((scr_x - width)/2)
            offset_y = 0
            
            self._scr.erase()
            # 绘制上边框
            self._scr.addstr(beg_y, beg_x, '╔' + '╦'.join(tmp) + '╗')
            for y in range(height - 2):
                if (y + 1) % (level + 1) == 0:
                    self._scr.addstr(beg_y + 1 + y, beg_x, '╠' + '╬'.join(tmp) + '╣')
                    offset_y -= 1
                #elif (y + 1) % 2 == 0:
                #    self._scr.addstr(beg_y + 1 + y, beg_x, '║' + '║'.join(tmp2) + '║')
                #    offset_y -= 1
                else:
                    offset_x = 0
                    for x in range(width):
                        
                        if x % (2*level) == 0:
                            self._scr.addch(beg_y + 1 + y, beg_x + x, '║')
                            offset_x -= 1
                        elif x % 2 == 0:
                            self._scr.addch(beg_y + 1 + y, beg_x + x, ' ')
                            offset_x -= 1
                        else:
                            realpos_x, realpos_y = x + offset_x, y + offset_y
                            nbr = overlay[realpos_x][realpos_y]
                            # 选中时的颜色和锁定时的颜色
                            color = None
                            if (realpos_x, realpos_y) == tuple(cursor_pos):
                                if gamegrid.is_nonlock(cursor_pos):
                                    color = curses.color_pair(2)
                                else:
                                    color = curses.color_pair(6)
                            else:
                                if gamegrid.is_nonlock((realpos_x, realpos_y)):
                                    color = curses.color_pair(1)
                                else:
                                    color = curses.color_pair(5)
                            color |= curses.A_UNDERLINE
                            if nbr == 0:
                                self._scr.addstr(beg_y + 1 + y, beg_x + x, ' ', color)
                            else:
                                self._scr.addstr(beg_y + 1 + y, beg_x + x, str(overlay[realpos_x][realpos_y]), color)
            self._scr.addstr(beg_y + height - 1, beg_x, '╚' + '╩'.join(tmp) + '╝')
            self._draw_version()
            
            self._scr.refresh()
            
            k = self._scr.getch()
            if k == ord('q'):
                confirm_text1 = '是否退出？退出后当前进度将会丢失'
                confirm_text2 = '输入大写Q确认，其他键取消'
                self._scr.addstr(scr_y - 4, self._get_center_pos(confirm_text1, 0)[1], confirm_text1)
                self._scr.addstr(scr_y - 3, self._get_center_pos(confirm_text2, 0)[1], confirm_text2)
                self._scr.refresh()
                if self._scr.getch() == ord('Q'):
                    self._game_state = 'aborted'
                    raise _SudokuBackToTitle()
            elif k == curses.KEY_UP:
                if cursor_pos[1] > 0:
                    cursor_pos[1] -= 1
            elif k == curses.KEY_DOWN:
                if cursor_pos[1] < level**2 - 1:
                    cursor_pos[1] += 1
            elif k == curses.KEY_LEFT:
                if cursor_pos[0] > 0:
                    cursor_pos[0] -= 1
            elif k == curses.KEY_RIGHT:
                if cursor_pos[0] < level**2 - 1:
                    cursor_pos[0] += 1
            elif ord('0') <= k <= ord('9'):
                if gamegrid.is_nonlock(tuple(cursor_pos)):
                    steps += 1
                    overlay[cursor_pos[0]][cursor_pos[1]] = int(chr(k))
            
            if gamegrid.check_overlay():
                self._scr.clear()
                lines = (
                    'You won the game!',
                    '所用时间: %fs'%(time.time() - beg_time),
                    f'使用步数: {steps}',
                    f'难度: {self._hardness}',
                    '按任意键继续......'
                )
                
                base_y = 5
                for t in lines:
                    self._scr.addstr(base_y, int((scr_x - self._get_string_width(t)) / 2), t)
                    base_y += 1
                self._game_state = 'win'
                self._scr.refresh()
                
                self._scr.getch()
                raise _SudokuBackToTitle()
        
    def _render_settings(self):
        settings = {
            'secret': {
                'label': '神秘设置',
                'action': (lambda x:not x)
            }
        }
        
        while True:
            scr_y, scr_x = self._scr.getmaxyx()
            
            k = self._scr.getch()
        
    def _render_input(self):
        pass
    
    def is_actived(self):
        return self._active
    
    def destroy(self):
        if self._actived:
            self.stop_render()
            self._actived = False
            self._win = None
            curses.endwin()
            
class SudokuLegacyRenderer:
    pass


if __name__ == '__main__':
    import pdb
    #pdb.set_trace()
    rend = SudokuRenderer()
    try:
        rend.render()
    except _SudokuStopGame as err:
        rend.destroy()
        print(err)
    except curses.error as err:
        rend.destroy()
        print('=========TRACEBACK START=========')
        traceback.print_exception(err)
        print('==========TRACEBACK END==========')
        print('curses遇到了一个?，造成此问题的原因可能是终端太小了。')
    except Exception as err:
        rend.destroy()
        print('=========TRACEBACK START=========')
        traceback.print_exception(err)
        print('==========TRACEBACK END==========')
        print('PySoduku遇到了亿个bug，请看看代码!阿里嘎多!')
    rend.destroy()