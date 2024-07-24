// compile with: c++ minesweeper.cc -o minesweeper-cc -lncursesw -std=c++20

#define NCURSES_WIDECHAR 1

#include <curses.h>
#include <cstdio>
#include <chrono>
#include <vector>
#include <sstream>
#include <fstream>
#include <random>
#include <memory>
#include <optional>
#include <cmath>
#include <utility>
#include <exception>
#include <algorithm>

const int ERR_TOO_MANY_MINES = 2;

const char *EVENT_ID_NONE = "none";
const char *EVENT_ID_KEYBOARD = "keyboard";
const char *EVENT_ID_INTERRUPT = "interrupt";
const char *EVENT_ID_REDRAW_ALL = "redraw_all";

const int OPEN_RESULT_BOMW = -1;
const int OPEN_RESULT_HAS_FLAG = 1;

const short COLOR_OPENED = 20;
const short COLOR_UNOPENED = 21;
const short COLOR_OPENED_SELECTED = 22;
const short COLOR_UNOPENED_SELECTED = 23;
const short COLOR_LBLUE = 24;
const short COLOR_ARRAY[9] = {
    COLOR_BLACK ,
    COLOR_LBLUE ,
    COLOR_GREEN ,
    COLOR_RED   ,
    COLOR_YELLOW,
    COLOR_BLACK  ,
    COLOR_BLACK ,
    COLOR_BLACK ,
    COLOR_BLACK
};

const int LIM_MAX_WIDTH = 128;

const short PAIR_UNOPENED = 20;
const short PAIR_UNOPENED_SELECTED = 21;
const short PAIR_OPENED_BASE = 22;
const short PAIR_OPENED_SELECTED_BASE = 31;

const wchar_t *num2chinese(uint8_t num) {
    switch (num) {
        case 1:
            return L"一";
        case 2:
            return L"二";
        case 3:
            return L"三";
        case 4:
            return L"四";
        case 5:
            return L"五";
        case 6:
            return L"六";
        case 7:
            return L"七";
        case 8:
            return L"八";
        default:
            return L"  ";
    }
}

class context;
class render_context;

using context_stack = std::vector<std::shared_ptr<context>>;
using myclock = std::chrono::steady_clock;
using namespace std::chrono_literals;

template<typename T>
inline void limited_sub(T &target, T max, T min = 0) {
    if (target <= min) {
        target = max;
    } else {
        target -= 1;
    }
}

template<typename T>
inline void limited_add(T &target, T max, T min = 0) {
    if (target >= max) {
        target = min;
    } else {
        target += 1;
    }
}

std::wstring fmt_duration(size_t dur_s) {
    std::wstringstream s;

#define tier(num, suffix) \
    if (dur_s >= (num)) { \
        s << (size_t)((float)dur_s / (num)) << (suffix); \
        dur_s %= num; \
    }

    tier(86400, L"天");
    tier(3600, L"时");
    tier(60, L"分");

    s << dur_s << L"秒";

#undef tier

    return s.str();
}

class block {
public:

    block() {
        is_opened = false;
        has_flag = false;
        type = TYPE_EMPTY;
    }

    enum inner_type {
        TYPE_EMPTY,
        TYPE_MINE,
    };

    bool has_flag;
    bool is_opened;
    int type;
    uint8_t num;
};

class grid {
public:

    grid(int width, int height) {
        resize(width, height);
    }

    int width() const { return _storage.size(); }
    int height() const { return _storage.at(0).size(); }

    block& locate(int x, int y) {
        return _storage.at(x).at(y);
    }

    const block& locate(int x, int y) const {
        return _storage.at(x).at(y);
    }

    void resize(int width, int height) {
        std::vector<std::vector<block>> outer_tmp;
        outer_tmp.reserve(width);

        for (int i1 = 0; i1 < width; i1++) {
            std::vector<block> tmp;
            tmp.reserve(height);
            for (int i2 = 0; i2 < height; i2++)
                tmp.emplace_back();
            outer_tmp.push_back(std::move(tmp));
        }

        _storage = outer_tmp;
    }

    int place_mines(
        int mine_number,
        std::optional<std::pair<int, int>> exclude_pos = std::optional<std::pair<int, int>>()
    ) {
        int width = this->width();
        int height = this->height();

        if (mine_number > width * height - 9) {
            return ERR_TOO_MANY_MINES;
        }

        std::random_device rd;
        std::default_random_engine rng(rd());
        std::uniform_int_distribution
            dist_x(0, width - 1), dist_y(0, height - 1);

        for (int i = 0; i < mine_number; i++) {
            int x = dist_x(rng);
            int y = dist_y(rng);
            auto &target = locate(x, y);

            int dx = 0;
            int dy = 0;
            if (exclude_pos.has_value()) {
                dx = std::abs(x - exclude_pos->first);
                dy = std::abs(y - exclude_pos->second);
            }

            if ((dx <= 1 && dy <= 1) || target.type != block::TYPE_EMPTY) {
                i -= 1;
                continue;
            } else {
                target.type = block::TYPE_MINE;
            }
        }

        return 0;
    }

    int try_open(int in_x, int in_y) {
        int width = this->width();
        int height = this->height();
        std::vector<std::pair<int, int>>
            waitlist{ std::make_pair(in_x, in_y) },
            tmp_waitlist;
        auto &target = locate(in_x, in_y);
        bool stop_spread = false;

        auto check_duplication = [&waitlist](int x, int y) -> bool {
            for (auto &pos : waitlist) {
                if (pos.first == x && pos.second == y) {
                    return true;
                }
            }
            return false;
        };

        tmp_waitlist.reserve(8);

        if (target.has_flag) {
            return OPEN_RESULT_HAS_FLAG;
        } else if (target.is_opened) {
            return 0;
        }

        if (target.type == block::TYPE_MINE) {
            return OPEN_RESULT_BOMW;
        }

        size_t index = 0;
        while (index < waitlist.size()) {
            auto &tmp1 = waitlist.at(index);
            int origin_x = tmp1.first;
            int origin_y = tmp1.second;
            uint8_t num_nearby_mines = 0;
            stop_spread = false;

            tmp_waitlist.clear();

            auto &tmp_t = locate(origin_x, origin_y);
            if (!tmp_t.has_flag)
                tmp_t.is_opened = true;

            // check the blocks around it
            for (int offset_x = -1; offset_x <= 1; offset_x += 1) {
                int x = origin_x + offset_x;
                if (x < 0 || width <= x)
                    continue;

                for (int offset_y = -1; offset_y <= 1; offset_y += 1) {
                    int y = origin_y + offset_y;
                    if (y < 0 || height <= y)
                        continue;

                    if (offset_x == 0 && offset_y == 0)
                        continue;

                    auto &t = locate(x, y);

                    if (t.type == block::TYPE_MINE) {
                        num_nearby_mines += 1;
                        if (!stop_spread) {
                            tmp_waitlist.clear();
                            stop_spread = true;
                        }
                    } else if (!stop_spread && !check_duplication(x, y)) {
                        tmp_waitlist.push_back(std::make_pair(x, y));
                    }
                }
            }

            tmp_t.num = num_nearby_mines;

            if (!tmp_waitlist.empty()) {
                waitlist.insert(waitlist.end(), tmp_waitlist.begin(), tmp_waitlist.end());
            }

            index += 1;
        }

        return 0;
    }

    void open_unchecked(int x, int y) {
        locate(x, y).is_opened = true;
    }

    bool is_succeed() const {
        int width = this->width();
        int height = this->height();
        for (int x = 0; x < width; x += 1) {
            for (int y = 0; y < height; y += 1) {
                auto &block = locate(x, y);
                if (block.type == block::TYPE_EMPTY && !block.is_opened)
                    return false;
            }
        }

        return true;
    }

protected:

    std::vector<std::vector<block>> _storage;
};

class event {
public:

    event(const char *id) : id(id) {}

    virtual void _placeholder() {}

    const char *id;
};

class keyboard_event : public event {
public:

    keyboard_event(wchar_t ch) : ch(ch), event(EVENT_ID_KEYBOARD) {}

    wchar_t ch;
};

class difficulty {
public:
    constexpr difficulty(int width, int height, int num_mines) :
        width(width),
        height(height),
        num_mines(num_mines)
    {}

    int &ref_by_index(int index) {
        switch (index) {
            case 0:
                return width;
            case 1:
                return height;
            case 2:
                return num_mines;
            default:
                throw std::out_of_range("index is out of range [0, 2]");
        }
        // this is unreachable!!!!!
        return width;
    }

    const int &ref_by_index(int index) const {
        switch (index) {
            case 0:
                return width;
            case 1:
                return height;
            case 2:
                return num_mines;
            default:
                throw std::out_of_range("index is out of range [0, 2]");
        }
        // this is unreachable!!!!!
        return width;
    }

    int possible_max_mines() const {
        int s = width * height;
        int tmp_result = static_cast<int>(static_cast<float>(s) * 0.9);

        return s - tmp_result < 9 ? s - 9 : tmp_result;
    }

    void ensure_mines_limit() {
        int max = possible_max_mines();
        if (num_mines > max)
            num_mines = max;
    }

    int width, height, num_mines;
};

class context {
public:

    context() : need_redraw(true) {}

    virtual void update(render_context &rctx, const event &event) = 0;

    bool need_redraw;
};

class render_context {
public:
    render_context(std::shared_ptr<context_stack> ctx_stack, WINDOW *win):
        ctx_stack(ctx_stack),
        win(win),
        is_request_clear(false)
    {}

    void pop_context() {
        request_clear();
        ctx_stack->pop_back();

        if (!ctx_stack->empty())
            ctx_stack->back()->need_redraw = true;
    }

    void push_context(std::shared_ptr<context> &&ctx) {
        request_clear();
        ctx_stack->emplace_back(std::move(ctx));
    }

    void request_clear() {
        is_request_clear = true;
    }

    std::shared_ptr<context_stack> ctx_stack;
    WINDOW *win;
    bool is_request_clear;
};

class game_context : public context {
public:

    game_context(difficulty d) :
        _game_grid(d.width, d.height),
        _cur_x(0), _cur_y(0),
        _base_x(0), _base_y(0),
        _game_over(false),
        _bottom_msg(nullptr),
        _last_redraw_time(myclock::now()),
        _difficulty(d),
        _num_flags(0),
        _first_click(true)
    {}

    virtual void update(render_context &rctx, const event &event) override {
        if (event.id == EVENT_ID_NONE && myclock::now() - _last_redraw_time < 400ms) {
            return;
        } else if (event.id == EVENT_ID_KEYBOARD) {
            auto &kb_event = dynamic_cast<const keyboard_event&>(event);

            handle_ch(kb_event.ch);
        }

        redraw_all(rctx.win, _base_x, _base_y);
    }

    void handle_ch(wchar_t ch) {
        int x = _cur_x, y = _cur_y;

        switch (ch) {
            case KEY_UP:
            case L'w':
            case L'W':
                limited_sub(_cur_y, _game_grid.height() - 1);
                break;
            case KEY_DOWN:
            case L's':
            case L'S':
                limited_add(_cur_y, _game_grid.height() - 1);
                break;
            case KEY_LEFT:
            case L'a':
            case L'A':
                limited_sub(_cur_x, _game_grid.width() - 1);
                break;
            case KEY_RIGHT:
            case L'd':
            case L'D':
                limited_add(_cur_x, _game_grid.width() - 1);
                break;
            case L'\n':
            case L' ':
                {
                if (_game_over)
                    return;

                if (_first_click) {
                    _begin_time = std::move(myclock::now());
                    _game_grid.place_mines(_difficulty.num_mines, std::optional(std::make_pair(x, y)));
                    _first_click = false;
                }

                int code = _game_grid.try_open(x, y);
                if (code != 0) {
                    _game_over = true;
                    if (code == OPEN_RESULT_BOMW) {
                        _bottom_msg = L"踩雷了，游戏结束!";
                    } else if (code == OPEN_RESULT_HAS_FLAG) {
                        _game_over = false;
                    } else {
                        _bottom_msg = L"o.o发生什么事了?";
                    }
                }

                if (_game_grid.is_succeed()) {
                    _game_over = true;
                    _bottom_msg = L"扫雷成功!";
                }

                if (_game_over) {
                    _end_time = std::move(myclock::now());
                }
                }

                break;
            case L'f':
            case L'F':
                {
                if (_game_over)
                    return;

                auto &block = _game_grid.locate(x, y);
                if (!block.is_opened) {
                    block.has_flag = !block.has_flag;
                }
                if (block.has_flag) {
                    _num_flags += 1;
                } else {
                    _num_flags -= 1;
                }
                }

                break;
        }
    }

    void redraw_all(WINDOW *win, int base_x, int base_y) {
        int width = _game_grid.width(), height = _game_grid.height();
        _last_redraw_time = myclock::now();

        for (int y = 0; y < height; y++) {
            wmove(win, base_y + y, base_x);
            for (int x = 0; x < width; x++) {
                auto &block = _game_grid.locate(x, y);

                bool is_selected = false;
                if (x == _cur_x && y == _cur_y) {
                    is_selected = true;
                }
                short pair = 0;
                if (block.is_opened) {
                    if (is_selected) {
                        pair = PAIR_OPENED_SELECTED_BASE;
                    } else {
                        pair = PAIR_OPENED_BASE;
                    }
                    pair += block.num;
                } else {
                    if (is_selected) {
                        pair = PAIR_UNOPENED_SELECTED;
                    } else {
                        pair = PAIR_UNOPENED;
                    }
                }

                const wchar_t *text = nullptr;
                if (block.has_flag) {
                    text = L"🚩";
                } else if (block.type == block::TYPE_MINE && (block.is_opened || _game_over)) {
                    text = L"💣";
                } else if (block.is_opened) {
                    text = num2chinese(block.num);
                } else {
                    text = L"  ";
                }

                attron(COLOR_PAIR(pair));

                waddwstr(win, text);

                attroff(COLOR_PAIR(pair));
            }
        }

        int by = _base_y + _game_grid.height();

        std::wstringstream s;

        s << L"地雷数: " << _difficulty.num_mines;
        s << L"\t旗子数: " << _num_flags;
        s << L"\t剩余: " << _difficulty.num_mines - _num_flags;

        wmove(win, by, _base_x);
        wclrtoeol(win);
        waddnwstr(win, s.view().data(), s.view().size());

        if (_begin_time.has_value()) {
            by += 1;
            auto time_now = myclock::now();

            if (_end_time.has_value())
                time_now = *_end_time;

            auto dur = std::chrono::duration_cast<std::chrono::seconds>(time_now - *_begin_time);
            auto time_str = fmt_duration(dur.count());

            wmove(win, by, _base_x);
            wclrtoeol(win);
            waddwstr(win, time_str.c_str());
        }

        if (_game_over) {
            by += 1;
            mvwaddwstr(win, by, _base_x, _bottom_msg);
            by += 1;
            mvwaddwstr(win, by, _base_x, L"按下Ctrl+C退出");
        }
    }

private:
    int _cur_x;
    int _cur_y;
    int _base_x;
    int _base_y;
    grid _game_grid;
    bool _game_over;
    bool _first_click;
    std::optional<myclock::time_point> _begin_time;
    std::optional<myclock::time_point> _end_time;
    myclock::time_point _last_redraw_time;
    difficulty _difficulty;
    int _num_flags;
    const wchar_t *_bottom_msg;
};

class difficulty_context : public context {
public:
    static constexpr int N_PRESETS = 4;

    static constexpr difficulty PRESETS[] = {
        difficulty(8, 8, 8),
        difficulty(16, 16, 16),
        difficulty(28, 20, 52)
    };

    difficulty_context() :
        index(0),
        custom_difficulty(difficulty(8, 8, 8)),
        custom_index(0)
    {}

    virtual void update(render_context &rctx, const event &event) override {
        if (event.id == EVENT_ID_NONE) {
            return;
        } else if (event.id == EVENT_ID_KEYBOARD) {
            auto &kb_event = dynamic_cast<const keyboard_event&>(event);

            switch (kb_event.ch) {
                case 9:
                    limited_add(index, N_PRESETS - 1);
                    break;

                case KEY_UP:
                    if (index == N_PRESETS - 1)
                        limited_sub(custom_index, 2);
                    break;

                case KEY_DOWN:
                    if (index == N_PRESETS - 1)
                        limited_add(custom_index, 2);
                    break;

                case KEY_LEFT:
                    if (index == N_PRESETS - 1) {
                        auto &target = custom_difficulty.ref_by_index(custom_index);
                        if (custom_index != 2) {
                            limited_sub(target, LIM_MAX_WIDTH, 4);
                        } else {
                            limited_sub(target, custom_difficulty.possible_max_mines(), 1);
                        }
                        custom_difficulty.ensure_mines_limit();
                    }
                    break;

                case KEY_RIGHT:
                    if (index == N_PRESETS - 1) {
                        auto &target = custom_difficulty.ref_by_index(custom_index);
                        if (custom_index != 2) {
                            limited_add(target, LIM_MAX_WIDTH, 4);
                        } else {
                            limited_add(target, custom_difficulty.possible_max_mines(), 1);
                        }
                        custom_difficulty.ensure_mines_limit();
                    }
                    break;

                case L'\n':
                    const difficulty *d = get_difficulty();

                    rctx.pop_context();
                    rctx.push_context(std::make_shared<game_context>(*d));

                    break;
            }

            rctx.request_clear();
        }

        redraw_all(rctx.win);
    }

    void redraw_all(WINDOW *win) {
        int width = getmaxx(win);

        static const wchar_t *PRESET_NAMES[] = {
            L"简单",
            L"中等",
            L"困难",
            L"自定义"
        };
        static int TW = 4;
        int base_x = (width - TW) / 2;

        for (int i = 0; i < N_PRESETS; i += 1) {
            short pair = 0;
            if (index == i) {
                pair = PAIR_UNOPENED_SELECTED;
            } else {
                pair = 0;
            }

            attron(COLOR_PAIR(pair));

            mvwaddwstr(win, 3 + i*3, base_x - 10, PRESET_NAMES[i]);

            attroff(COLOR_PAIR(pair));
        }

        mvwaddwstr(win, 3 + N_PRESETS*3, base_x - 10, L"按下TAB键切换");
        mvwaddwstr(win, 4 + N_PRESETS*3, base_x - 10, L"按下ENTER键确认");

        static const wchar_t *PREFIXES[] = {
            L"宽",
            L"高",
            L"地雷数"
        };
        int i;

        auto fmt = [this, win](auto arg1) {
            std::wstringstream s;
            if (index == N_PRESETS - 1) {
                s << L"<- " << arg1 << L" ->";
            } else {
                s << arg1;
            }
            waddnwstr(win, s.view().data(), s.view().size());
        };

        for (i = 0; i < 3; i += 1) {
       
            wmove(win, 3 + i*2, base_x + 4);
            waddwstr(win, PREFIXES[i]);
            waddwstr(win, L": ");
       
            if (i == custom_index && index == N_PRESETS - 1) {
                attron(COLOR_PAIR(PAIR_UNOPENED_SELECTED));
            }

            const difficulty *d = get_difficulty();

            fmt(d->ref_by_index(i));
       
            if (i == custom_index && index == N_PRESETS - 1) {
                attroff(COLOR_PAIR(PAIR_UNOPENED_SELECTED));
            }
        }
    }

    const difficulty *get_difficulty() const {
        if (index == N_PRESETS - 1) {
            return &custom_difficulty;
        } else {
            return &PRESETS[index];
        }
    }

    int index;
    int custom_index;
    difficulty custom_difficulty;
};

class menu_context : public context {
public:
    menu_context() : _index(0) {}

    virtual void update(render_context &rctx, const event &event) override {
        if (event.id == EVENT_ID_NONE) {
            return;
        } else if (event.id == EVENT_ID_KEYBOARD) {
            auto &kb_event = dynamic_cast<const keyboard_event &>(event);

            switch (kb_event.ch) {
                case KEY_UP:
                    if (_index > 0)
                        _index -= 1;
                    break;

                case KEY_DOWN:
                    if (_index < 1)
                        _index += 1;
                    break;
                case L'\n':
                    enter(rctx);
                    break;
            }
        }

        redraw_all(rctx.win);
    }

    void enter(render_context &rctx) {
        switch (_index) {
            case 0:
                rctx.push_context(std::make_shared<difficulty_context>());
                break;
            case 1:
                rctx.pop_context();
                break;
        }
    }

    void redraw_all(WINDOW *win) {
        int width = getmaxx(win);

        const wchar_t *TEXTS[] = { L"开始游戏", L"退出游戏" };
        const int tw = 8;

        for (int i = 0; i < 2; i += 1) {
            short pair;

            if (_index == i) {
                pair = PAIR_UNOPENED_SELECTED;
            } else {
                pair = 0;
            }

            attron(COLOR_PAIR(pair));

            mvwaddwstr(win, 3 + i * 5, (width - tw) / 2, TEXTS[i]);

            attroff(COLOR_PAIR(pair));
        }
    }

protected:
    int _index;
};

class game {
public:

    game() :
        _ctx_stack(std::make_shared<context_stack>()), _win(stdscr),
        //_debug_flag(true),
        _last_size(1, 1)
    {
        _ctx_stack->emplace_back(std::make_shared<menu_context>());
    }

    int run() {
        bool need_clear = false;

        while (!_ctx_stack->empty()) {
            render_context rctx(_ctx_stack, _win);
            auto ctx = _ctx_stack->back();

            int width, height;
            getmaxyx(_win, height, width);

            if (need_clear) {
                werase(_win);
                ctx->need_redraw = true;
                need_clear = false;
            }

            wint_t k;
            get_wch(&k);
/*
            if (_debug_flag) {
                int height = getmaxy(_win);

                if (k != 0) {
                    if (_keyboard_cache.size() >= 5) {
                        _keyboard_cache.erase(0, 1);
                    }
                    _keyboard_cache.push_back((wchar_t)k);

                    std::wstringstream tmp_stream;
                    std::ranges::for_each(
                        _keyboard_cache.crbegin(), _keyboard_cache.crend(),
                        [&tmp_stream](wchar_t ch) {
                            tmp_stream << (uint32_t)ch << ' ';
                        });

                    mvwaddnwstr(
                        _win,
                        height - 1, 0,
                        tmp_stream.view().data(),
                        tmp_stream.view().size());
                }
            }
*/
            // check events
            if (k != 0) {
                if (k == 3) {
                    ctx->update(rctx, event(EVENT_ID_INTERRUPT));
                    rctx.pop_context();
                } else {
                    keyboard_event kb_event((wchar_t)k);
                    ctx->update(rctx, kb_event);
                }
            } else {
                event e(EVENT_ID_NONE);
                ctx->update(rctx, e);
            }

            if (ctx->need_redraw) {
                ctx->need_redraw = false;
                ctx->update(rctx, event(EVENT_ID_REDRAW_ALL));
            }

            need_clear = rctx.is_request_clear;

            if (_last_size.first != width || _last_size.second != height) {
                _last_size = std::pair<int, int>(width, height);
                need_clear = true;
            }

            wrefresh(_win);
        }

        return 0;
    }

protected:

    //bool _debug_flag;
    //std::wstring _keyboard_cache;
    std::pair<int, int> _last_size;
    std::shared_ptr<context_stack> _ctx_stack;
    WINDOW *_win;
};

int main() {
    setlocale(LC_ALL, "");

    initscr();
    raw();
    noecho();
    nodelay(stdscr, TRUE);
    start_color();
    keypad(stdscr, TRUE);
    curs_set(0);

    init_color(COLOR_UNOPENED, 900, 900, 900);
    init_color(COLOR_OPENED, 500, 500, 500);
    init_color(COLOR_OPENED_SELECTED, 650, 650, 650);
    init_color(COLOR_UNOPENED_SELECTED, 750, 750, 750);
    init_color(COLOR_LBLUE, 650, 740, 900);

    init_pair(PAIR_UNOPENED, COLOR_BLACK, COLOR_UNOPENED);
    init_pair(PAIR_UNOPENED_SELECTED, COLOR_BLACK, COLOR_UNOPENED_SELECTED);

    for (int i = 0; i < 8; i += 1) {
        init_pair(PAIR_OPENED_BASE + i, COLOR_ARRAY[i], COLOR_OPENED);
        init_pair(PAIR_OPENED_SELECTED_BASE + i, COLOR_ARRAY[i], COLOR_OPENED_SELECTED);
    }

    game g;
    int result = g.run();

    curs_set(1);
    keypad(stdscr, FALSE);
    nodelay(stdscr, FALSE);
    echo();
    noraw();
    endwin();

    return result;
}
