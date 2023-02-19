#include <string>
#include <exception>
#include <locale>
#include <cstdio>
#include <thread>
#include <chrono>
#include <vector>
#include <random>

#include "ncurses.h"
#include "unistd.h"
#include "unicode/utypes.h"
#include "unicode/ucnv.h"

bool UI_LOCK = false;

int get_string_width(const std::string &str) {
    return 1;
}

class overflow_error : public std::exception {
public:
    overflow_error(const std::string &reason) : str(reason) {
    }
    overflow_error(int x, int y) {
        char buf[33] = {0};
        if (snprintf(buf, 32, "Position (%d,%d) is out of grid", x, y) == -1) {
            str.assign("Position is out of screen");
        } else {
            str.assign(buf);
        }
    }
    
    const char* what() {
        return str.c_str();
    }
    
protected:
    std::string str;
};

class cell {
public:
    cell() : _status( cell::Empty ), _direction(DNone), _ndirection(DNone) {}
    cell(int status) : _status(status), _direction(DNone), _ndirection(DNone) {}
    
    enum _cell_status {
        Empty,
        Apple,
        Wall,
        SnakeHead,
        SnakeBody,
    } cell_status;
    
    enum _cell_directions {
        DNone,
        DUp,
        DRight,
        DDown,
        DLeft,
    } cell_directions;
    
    int get_status() { return _status; }
    int set_status(int nval) {
        int oval = _status;
        _status = nval;
        return oval;
    }
    
    int get_direction() { return _direction; }
    int set_direction(int nval) {
        int oval = _direction;
        _direction = nval;
        return oval;
    }
    
    int get_next_direction() { return _ndirection; }
    int set_next_direction(int nval) {
        int oval = _ndirection;
        _ndirection = nval;
        return oval;
    }
    
protected:
    int _status;
    int _direction;
    int _ndirection;
};




int get_opposite_direction(int d) {
    switch(d) {
    case cell::DDown:
        return cell::DUp;
    case cell::DUp:
        return cell::DDown;
    case cell::DRight:
        return cell::DLeft;
    case cell::DLeft:
        return cell::DRight;
    default:
        return cell::DNone;
    }
}



class position {
public:
    position(int x, int y) : x(x), y(y) {}
    position(const position &oldval) {
        x = oldval.x;
        y = oldval.y;
    }
    position(position &&rval) {
        x = rval.x;
        y = rval.y;
    }
    
    position &operator=(const position &nval) {
        x = nval.x;
        y = nval.y;
        return *this;
    }
    
    void move(int direction) {
        switch(direction) {
        case cell::DUp:
            y -= 1;
            break;
        case cell::DDown:
            y += 1;
            break;
        case cell::DLeft:
            x -= 1;
            break;
        case cell::DRight:
            x += 1;
            break;
        }
    }
    
    int x, y;
};



class grid {
public:
    grid(int width, int height) :
        _width(width),
        _height(height),
        _head_pos(-1, -1) {
        _grid = new cell[width * height]();
    }
    
    ~grid() {
        delete[] _grid;
    }
    
    int get_width() { return _width; }
    int get_height() { return _height; }
    
    cell& get_head() { return at(_head_pos); }
    position get_head_pos() { return _head_pos; }
    
    cell& at(const position& pos) {
        if ( pos.y >= _height || pos.x >= _width || pos.x < 0 || pos.y < 0 )
            throw std::out_of_range("坐标超出范围");
        return _grid[pos.y * _width + pos.x];
    }
    
    cell& operator[](const position& pos) {
        return at(pos);
    }
    
    int move(const position &pos_, int direction, bool force) {
        position pos(pos_);
        cell &old = at(pos);
        
        if ( pos.x == _head_pos.x && pos.y == _head_pos.y )
            _head_pos.move(direction);
            
        pos.move(direction);
        
        cell &target = at(pos);
        
        if ( !force && target.get_status() != cell::Empty )
            return target.get_status();
        
        target = old;
        target.set_next_direction(get_opposite_direction(direction));
        old.set_status( cell::Empty );
        
        return cell::Empty;
    }
    
    void put_snake(int length) {
        int x = static_cast<int>(get_width() / 2), y = static_cast<int>(get_height() / 2);
        cell &center = at(position(x, y));
        center.set_status( cell::SnakeHead );
        center.set_direction( cell::DDown );
        _head_pos.x = x;
        _head_pos.y = y;
        _hided_bodies = length - 1;
    }
    
    cell* get_hided_body() {
        if ( _hided_bodies > 0 ) {
            cell* ret = new cell( cell::SnakeBody );
            _hided_bodies -= 1;
            return ret;
        } else {
            return nullptr;
        }
    }
    
    int add_apple() {
        return add_apple(false);
    }
    int add_apple(bool force) {
        static std::uniform_int_distribution<int> wr(0, _width - 1), hr(0, _height - 1);
        static std::default_random_engine r(time(NULL));
        
        bool haveblank = false;
        for (int x = 0; x < _width; x += 1) {
            for (int y = 0; y < _height; y += 1) {
                int s = at(position(x, y)).get_status();
                if ( s == cell::Empty ) {
                    haveblank = true;
                } else if ( s == cell::Apple && !force) {
                    return 1;
                }
            }
        }
        
        if ( !haveblank )
            return 2;
        position pos(0, 0);
        do {
            pos.x = wr(r);
            pos.y = hr(r);
        } while ( at(pos).get_status() != cell::Empty);
        
        at(pos).set_status( cell::Apple );
        
        return 0;
    }
    
protected:
    int _width;
    int _height;
    
    position _head_pos;
    cell* _grid;
    int _hided_bodies;
};



class game {
public:
    game(grid* v) :
        _grid(v),
        cfg_fix_rect(false),
        cfg_hardness(3),
        _inited(false) {
        _grid->put_snake(3);
    }
    
    void init() {
        if (!_inited) {
            int width = _grid->get_width() * (cfg_fix_rect ? 2 : 1);
            int height = _grid->get_height();
            
            _scr = newwin(height + 2, width + 2, 5, static_cast<int>((COLS - width + 2) / 2));
            if (_scr == NULL)
                throw overflow_error(height + 2, width + 2);
            nodelay(_scr, 1);
            curs_set(0);
            keypad(stdscr, 1);
            keypad(_scr, 1);
            
            _inited = true;
        }
    }
    
    void render() {
        if (!_inited)
            throw "Not inited";
        const char *blank, *apple, *wall, *body, *head;
        if (cfg_fix_rect) {
            blank = "  ";
            apple = "🍎";
            wall = "[]";
            body = "🍞";
            head = "🐍";
        } else {
            blank = " ";
            apple = "@";
            wall = "|";
            body = "#";
            head = "+";
        }
        int last_x = 0, last_y = 0, win_x, win_y, \
            width = _grid->get_width(), \
            height = _grid->get_height(), \
            k1 = cfg_fix_rect ? 2 : 1,
            k = -1,
            score = 0;
            
        std::chrono::microseconds frame_time(static_cast<int64_t>(1 / static_cast<double>(cfg_hardness) * 1000000)), timer(0);
        while (1) {
        
            static auto process_key = [&]() -> int {
                cell &headc = _grid->get_head();
                int headd = headc.get_direction();
                k = wgetch(_scr);
                if ( k == 'q' ) {
                    return 1;
                } else if ( k == KEY_UP && headd != cell::DDown ) {
                    headc.set_direction(cell::DUp);
                } else if ( k == KEY_RIGHT && headd != cell::DLeft ) {
                    headc.set_direction(cell::DRight);
                } else if ( k == KEY_DOWN && headd != cell::DUp ) {
                    headc.set_direction(cell::DDown);
                } else if ( k == KEY_LEFT && headd != cell::DRight ) {
                    headc.set_direction(cell::DLeft);
                } else if ( k == 'c' ) {
                    /*
                    position tmp = _grid->get_head_pos();
                    tmp.move(_grid->get_head().get_direction());
                    _grid->at(tmp).set_status( cell::Apple );
                    */
                    _grid->add_apple(true);
                }
                return 0;
            };
            
            auto beg_time = std::chrono::steady_clock::now();
            
            if ( timer > frame_time ) {
                timer = std::chrono::microseconds(0);
            
                win_x = COLS < width*k1 ? 0 : static_cast<int>((COLS - width*k1 + 2) / 2);
                win_y = LINES < height*k1 ? 0 : 5;
                if (win_x != last_x || win_y != last_y) {
                    mvwin(_scr, win_y, win_x);
                    last_x = win_x;
                    last_y = win_y;
                }
                
                if (process_key())
                    break;
                
                // 蛇的运动
                // 将头移动
                bool skip_move_body = false;
                cell &head_cell = _grid->get_head();
                position pos = _grid->get_head_pos();
                int nd = head_cell.get_next_direction();
                int d = head_cell.get_direction();
                
                try {
                    int ret = _grid->move(pos, head_cell.get_direction(), false);
                    cell tmp_cell;
                    switch(ret) {
                    case cell::Apple:
                        skip_move_body = true;
                        tmp_cell = head_cell;
                        tmp_cell.set_status(cell::SnakeBody);
                        _grid->move(pos, head_cell.get_direction(), true);
                        _grid->at(pos) = tmp_cell;
                        score += 1;
                        break;
                    case cell::Wall:
                    case cell::SnakeBody:
                    case cell::SnakeHead:
                        throw overflow_error("Game over");
                    }
                } catch(overflow_error &err) {
                    _render_gameover(err.what());
                    return;
                }
                
                cell* body_cell = _grid->get_hided_body();
                if ( body_cell != nullptr && !skip_move_body ) {
                    // 放置隐藏的身体
                    body_cell->set_next_direction(nd);
                    body_cell->set_direction(d);
                    _grid->at(pos) = *body_cell;
                    delete body_cell;
                } else if ( !skip_move_body ) {
                    while ( nd != cell::DNone ) {
                        pos.move(nd);
                        try {
                            cell &src = _grid->at(pos);
                            if ( src.get_status() != cell::SnakeBody )
                                break;
                            _grid->move(pos, get_opposite_direction(nd), true);
                            nd = src.get_next_direction();
                        } catch(overflow_error &err) {
                            _render_gameover(err.what());
                            return;
                        }
                    }
                }
                
                _grid->add_apple();
                
                werase(_scr);
                erase();
                
                wborder(_scr, 0, 0, 0, 0, 0, 0, 0, 0);
                
                // 输出所有内容
                for ( int x = 0; x < width; x += 1 ) {
                    for ( int y = 0; y < height; y += 1 ) {
                        int realx = x * k1 + 1;
                        int realy = y + 1;
                        switch(_grid->at(position(x, y)).get_status()) {
                        case cell::Empty:
                            mvwaddstr(_scr, realy, realx, blank);
                            break;
                        case cell::Apple:
                            mvwaddstr(_scr, realy, realx, apple);
                            break;
                        case cell::Wall:
                            mvwaddstr(_scr, realy, realx, wall);
                            break;
                        case cell::SnakeBody:
                            mvwaddstr(_scr, realy, realx, body);
                            break;
                        case cell::SnakeHead:
                            mvwaddstr(_scr, realy, realx, head);
                            break;
                        }
                    }
                }
                // 判断屏幕尺寸，输出分数和运行时间
                //strftime
                
                refresh();
                wrefresh(_scr);
            } else {
                if (process_key())
                    break;
            }
                
            auto end_time = std::chrono::steady_clock::now();
            
            timer += std::chrono::duration_cast<std::chrono::microseconds>(end_time - beg_time);
        } // while (1)
    } // void _render_game()
    
    void _render_gameover(const char *reason) {
        WINDOW* win;
    }
    
protected:
    grid* _grid;
    WINDOW* _scr;
    
    bool _inited;

public:
    
    bool cfg_fix_rect;
    int cfg_fps;
    int cfg_hardness;
};



void endgame() {
    endwin();
}

int main() {
    atexit(&endgame);
    
    setlocale(LC_ALL, "");
    
    initscr();
    cbreak();
    noecho();
    
    grid this_grid(20, 20);
    game no_game_no_life(&this_grid);
    no_game_no_life.cfg_fix_rect = true;
    no_game_no_life.cfg_hardness = 6;
    no_game_no_life.init();
    
    try {
        no_game_no_life.render();
    } catch(std::out_of_range &err) {
        
    }
    
    return EXIT_SUCCESS;
}