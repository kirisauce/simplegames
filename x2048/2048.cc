#include <ncurses.h>
#include <string>
#include <uchar.h>
#include <stringpiece.h>
#include <unistr.h>
#include <iostream>
#include <exception>
#include <functional>
#include <locale.h>
#include <random>
#include <ctime>
#include <cstring>
#include <utility>
#include <chrono>
#include <thread>

#define CALC_CENTER_BEGIN(scrwidth, strwidth) (int(((scrwidth) - (strwidth)) / 2))
#define CENTER_BEGIN(scrwidth, str) CALC_CENTER_BEGIN(scrwidth, x2048::get_string_width(str))

#define PAIR_HIGHLIGHT 1
#define PAIR_DIALOG 2
#define PAIR_GREEN_TEXT 3

#define COLOR_GRAY 9

#define GRID_SIZE 5

static const char *PROGRAM = "x2048";
static const char *AUTHOR  = "xuanyeovo";
static const char *LICENSE = "MIT LICENSE";

namespace x2048 {
    
    using i32 = int32_t;
    using i64 = int64_t;
    using u32 = uint32_t;
    using u64 = uint64_t;
    using f32 = float;
    using f64 = double;

    bool AMBIGUOUS_AS_WIDE = false;

    std::default_random_engine rand(time(NULL));

    auto get_string_width(const char *rawstr) {
        auto str = icu::UnicodeString::fromUTF8(icu::StringPiece(rawstr));
        auto len = str.length();
        i32 width = 0;
        for( int i = 0; i < len; i++) {
            switch((UEastAsianWidth)u_getIntPropertyValue(str.charAt(i), UCHAR_EAST_ASIAN_WIDTH)) {
            case U_EA_NEUTRAL: case U_EA_HALFWIDTH: case U_EA_NARROW:
                width += 1;
                break;
            case U_EA_FULLWIDTH: case U_EA_WIDE:
                width += 2;
                break;
            default:
                width += AMBIGUOUS_AS_WIDE ? 2 : 1;
                break;
            }
        }
        return width;
    }

    auto get_string_width(const std::string &str) {
        return get_string_width(str.c_str());
    }

    auto waddstrcenter(WINDOW *win, int y, const char *str) {
        return mvwaddstr(win, y, CENTER_BEGIN(getmaxx(win), str), str);
    }

    void wfill(WINDOW *win, int x1, int y1, int x2, int y2, const char *str) {
        if( x1 > x2 )
            std::swap(x1, x2);
        if( y1 > y2 )
            std::swap(y1, y2);

        for( int x = x1; x <= x2; x += 1 ) {
            for( int y = y1; y <= y2; y += 1 ) {
                mvwaddstr(win, y, x, str);
            }
        }
    }

    template<typename T>
    T get_max(T x) {
        return x;
    }

    template<typename T, typename... argsT>
    T get_max(T x1, T x2, argsT... args) {
        T tmp = get_max(x2, args...);
        return x1 > tmp ? x1 : tmp;
    }

    enum class DIRECTION {
        UP,
        DOWN,
        RIGHT,
        LEFT,
    };





    class GameStop : public std::exception {
    public:
        GameStop(int code, const std::string &reason) : mCode(code), mReason(reason) {}

        int code() const noexcept {
            return mCode;
        }

        const char *what() const noexcept {
            return mReason.c_str();
        }

    private:
        int mCode;
        std::string mReason;
    };



    class GameOver {
    public:
        GameOver(i64 s, const std::string &m, std::chrono::steady_clock::duration used_time) : mScore(s), mMsg(m), mUsedTime(used_time) {}

        const char *what() const noexcept {
            return mMsg.c_str();
        }

        i64 score() const noexcept {
            return mScore;
        }

    private:
        i64 mScore;
        std::string mMsg;
        std::chrono::steady_clock::duration mUsedTime;
    };



    /// 网格类，存储数字
    /// Example: 创建一个大小为4 x 3的网格并随机写入数字8
    ///   Grid g(4, 3);
    ///   g.generate(8);
    class Grid {
    public:
        using storage_t = i64;
        Grid(int w, int h) : mWidth(w), mHeight(h), mGrid(nullptr), mScore(0) {
            reset();
        }

        ~Grid() {
            delete mGrid;
            mGrid = nullptr;
        }

        void reset(int w, int h) {
            mWidth = w;
            mHeight = h;
            reset();
        }

        void reset() {
            delete mGrid;
            mGrid = new storage_t[mWidth * mHeight];
            memset((void*)mGrid, 0, mWidth * mHeight * sizeof(storage_t));

            score() = 0;
        }

        bool is_full() const {
            bool have_blank = false;
            for( int i = 0; i < mWidth * mHeight; i++ ) {
                if( mGrid[i] == 0 ) {
                    have_blank = true;
                    break;
                }
            }
            return !have_blank;
        }

        bool is_fail() const {
            if( !is_full() )
                return false;
            for( int x = 0; x < width(); x++ ) {
                for( int y = 0; y < height(); y++ ) {
                    auto cur = get(x, y);
                    if( y > 0 && cur == get(x, y - 1) )
                        return false;
                    if( y < width() - 1 && cur == get(x, y + 1) )
                        return false;
                    if( x > 0 && cur == get(x - 1, y) )
                        return false;
                    if( x < height() - 1 && cur == get(x + 1, y) )
                        return false;
                }
            }
            return true;
        }

        /// Put specified value into the empty randomly.
        /// Return true if there is not any empty which can be filled.
        bool generate(const storage_t &targetval) {
            static std::uniform_int_distribution<int> pos_dist(0, mWidth * mHeight - 1);
            if( is_full() )
                return true;

            while(true) {
                storage_t &val = mGrid[pos_dist(rand)];
                if( val == 0 ) {
                    val = targetval;
                    break;
                }
            }

            return false;
        }

        bool generate_randomly() {
            static std::uniform_int_distribution<int> false_1i4(0, 3);
            if( !false_1i4(rand) ) {
                if( !false_1i4(rand) ) {
                    if( !false_1i4(rand) ) {
                        return generate(16);
                    } else {
                        return generate(8);
                    }
                } else {
                    return generate(4);
                }
            } else {
                return generate(2);
            }
        }

        int slide(int x, int y, DIRECTION dire, bool skip_merge) {
            auto &cur = get(x, y);
            int coord = 1;

            if( cur == 0 )
                return -1;
            while(true) {
                try {
                    auto &target = __get_dire(x, y, dire, coord);
                    if( target != 0 ) {
                        if( skip_merge ) {
                            throw std::out_of_range("");
                        } else if( target == cur ) {
                            target *= 2;
                            cur = 0;
                            return target;
                        } else {
                            throw std::out_of_range("");
                        }
                    }
                } catch(std::out_of_range & e) {
                    if( coord > 1 ) {
                        auto &target = __get_dire(x, y, dire, coord - 1);
                        target = cur;
                        cur = 0;
                        return 0;
                    } else {
                        return -1;
                    }
                }
                coord++;
            }
        }

        /// 向一个方向合并格子
        /// 将此次操作获得的分数累加到score上并返回
        /// @param dire 将要合并的方向
        /// @param have_motions_out 如果有任何操作将会被设置为true，否则为false，可以为nullptr
        i64 merge(DIRECTION dire, bool *have_motions_out = nullptr) {
            i64 increase = only_merge(dire, have_motions_out);
            if( increase > 0 )
                mScore += increase;
            return increase;
        }

        /// 仅合并格子
        /// 不累加score，将获得的分数返回
        /// @param dire 将要合并的方向
        /// @param have_motions_out 如果有任何操作将会被设置为true，否则为false，可以为nullptr
        i64 only_merge(DIRECTION dire, bool *have_motions_out = nullptr) {
            i64 new_score = 0; // 增加的分数
            bool skip_merge = false;
            bool have_motions = false;

            #define __iter(iname, maxi) for( int iname = 0; iname < (maxi); iname++)

            // 合并阶段
            switch(dire) {
            case DIRECTION::UP:
                __iter(x, width()) {
                    skip_merge = false;
                    __iter(y, height()) {
                        i64 r = slide(x, y, dire, skip_merge);
                        if( r > 0 ) {
                            skip_merge = true;
                            new_score += r;
                        }
                        if( r >= 0 )
                            have_motions = true;
                    }
                }
                break;
            case DIRECTION::DOWN:
                __iter(x, width()) {
                    skip_merge = false;
                    __iter(y_i, height()) {
                        int y = height() - 1 - y_i;
                        i64 r = slide(x, y, dire, skip_merge);
                        if( r > 0 ) {
                            skip_merge = true;
                            new_score += r;
                        }
                        if( r >= 0 )
                            have_motions = true;
                    }
                }
                break;
            case DIRECTION::RIGHT:
                __iter(y, height()) {
                    skip_merge = false;
                    __iter(x_i, width()) {
                        int x = width() - 1 - x_i;
                        i64 r = slide(x, y, dire, skip_merge);
                        if( r > 0 ) {
                            skip_merge = true;
                            new_score += r;
                        }
                        if( r >= 0 )
                            have_motions = true;
                    }
                }
                break;
            case DIRECTION::LEFT:
                __iter(y, height()) {
                    skip_merge = false;
                    __iter(x, width()) {
                        i64 r = slide(x, y, dire, skip_merge);
                        if( r > 0 ) {
                            skip_merge = true;
                            new_score += r;
                        }
                        if( r >= 0 )
                            have_motions = true;
                    }
                }
                break;
            }


            if( have_motions_out )
                *have_motions_out = have_motions;
            return have_motions ? new_score : -1;
        }

        i64 &score() {
            return mScore;
        }

        storage_t &get(int x, int y) {
            if( x >= mWidth || x < 0 || y < 0 || y >= mHeight )
                throw std::out_of_range("Position X " + std::to_string(x) + " Y " + std::to_string(y) + " is out of range");
            return mGrid[x + y * mWidth];
        }

        storage_t get(int x, int y) const {
            if( x >= mWidth || x < 0 || y < 0 || y >= mHeight )
                throw std::out_of_range("Position X " + std::to_string(x) + " Y " + std::to_string(y) + " is out of range");
            return mGrid[x + y * mWidth];
        }

        void put(int x, int y, const storage_t &val) {
            get(x, y) = val;
        }

        int width() const {
            return mWidth;
        }

        int height() const {
            return mHeight;
        }

    private:
        int mWidth;
        int mHeight;
        storage_t *mGrid;

        i64 mScore;

        storage_t &__get_dire(int x, int y, DIRECTION dire, int coord = 1) {
            switch(dire) {
            case DIRECTION::UP:
                y -= coord;
                break;
            case DIRECTION::DOWN:
                y += coord;
                break;
            case DIRECTION::RIGHT:
                x += coord;
                break;
            case DIRECTION::LEFT:
                x -= coord;
                break;
            }
            return get(x, y);
        }

        DIRECTION __opposite_dire(DIRECTION dire) {
            switch(dire) {
            case DIRECTION::UP:
                return DIRECTION::DOWN;
            case DIRECTION::DOWN:
                return DIRECTION::UP;
            case DIRECTION::RIGHT:
                return DIRECTION::LEFT;
            case DIRECTION::LEFT:
                return DIRECTION::RIGHT;
            default:
                return DIRECTION::UP;
            }
        }
    };



    /// 执行游戏的类，自适应WINDOW大小
    /// 创建时会为窗口及TTY设置一些参数并调用savetty()，销毁时调用resetty()
    /// 以下是可配置的变量(懒得做Property，所以在游戏运行时请勿更改)
    /// config_width    - 游戏网格宽度
    /// config_height   - 游戏网格高度
    /// config_fix_rect - 宽度增加以使网格为正方形
    /// config_size     - 单个格子边长，若开启config_fix_rect则宽度乘2
    class Game {
    public:
        Game(WINDOW *_win = stdscr, int width = 4, int height = 6) : mWin(_win), mGrid(width, height), config_width(width), config_height(height), config_fix_rect(true), config_size(GRID_SIZE) {
            savetty();
            keypad(mWin, 1);
            scrollok(mWin, 0);
            nodelay(mWin, 0);
            noecho();
            cbreak();
            curs_set(0);

            start_color();
            use_default_colors();
            init_color(COLOR_GRAY, 0xa0, 0xa0, 0xa0);
            init_pair(PAIR_HIGHLIGHT, COLOR_BLACK, COLOR_WHITE);
            init_pair(PAIR_DIALOG, COLOR_RED, COLOR_GRAY);
            init_pair(PAIR_GREEN_TEXT, COLOR_GREEN, 0);
        }

        ~Game() {
            resetty();
        }

        struct choice_t {
            choice_t(const std::string &a, double b, const std::string &k, std::function<void()> cb) : text(a), ratio(b), match_keys(k), callback(cb) {}
            std::string text;
            double ratio;
            std::string match_keys;
            std::function<void ()> callback;
        };
        
        void render_title() {
            static const char *TITLE = "X2048!";
            //static const int TITLE_WIDTH = get_string_width(TITLE);

            constexpr int CHOICES_NBR = 3;
            static choice_t CHOICES[CHOICES_NBR] = {
                choice_t("开始游戏(A)", 0.3, "Aa", std::bind(&Game::render_game, this)),
                choice_t("设置(S)",     0.5, "Ss", std::bind(&Game::render_settings, this)),
                choice_t("退出游戏(Q)", 0.7, "Qq", std::bind(&Game::stop, this))
            };

            int select = 0;
            int k = 0;

            while(1) {
                werase(mWin);

                // 绘制标题和Debug信息
                waddstrcenter(mWin, int(getmaxy(mWin) * 0.1), TITLE);
                //std::string DEBUG_MSG = "KEY=" + std::to_string(k);
                //mvwaddstr(mWin, getmaxy(mWin) - 1, CENTER_BEGIN(getmaxx(mWin), DEBUG_MSG), DEBUG_MSG.c_str());

                // 绘制选项
                for( int i = 0; i < CHOICES_NBR; i++ ) {
                    auto &curr = CHOICES[i];
                    if( curr.text.empty() )
                        continue;
                    if( select == i )
                        wattron(mWin, COLOR_PAIR(PAIR_HIGHLIGHT));
                    waddstrcenter(mWin, int(getmaxy(mWin) * curr.ratio), curr.text.c_str());
                    if( select == i )
                        wattroff(mWin, COLOR_PAIR(PAIR_HIGHLIGHT));
                }

                wrefresh(mWin);

                k = getch();
                // 选项的Callback
                for( int i = 0; i < CHOICES_NBR; i++ ) {
                    auto &curr = CHOICES[i];
                    for( auto c : curr.match_keys ) {
                        if( k == c )
                            curr.callback();
                    }
                }
                // 方向键
                switch(k) {
                case KEY_UP:
                    if( 0 < select )
                        select -= 1;
                    break;
                case KEY_DOWN:
                    if( select < CHOICES_NBR - 1 )
                        select += 1;
                    break;
                case '\x02':
                    static std::string STR = "哔哩哔哩干杯！";
                    static bool first = true;
                    if( STR.empty() )
                        throw GameStop(233, "干杯！");
                    if( first ) {
                        first = false;
                        TITLE = STR.c_str();
                    } else {
                        STR.pop_back();
                        TITLE = STR.c_str();
                    }
                case KEY_ENTER: case 10: case 13:
                    CHOICES[select].callback();
                    break;
                }
            }
        } // void render_title()

        void render_game() {
            bool cond = true;
            bool dbg = false;
            std::chrono::milliseconds frametime(10);
            auto timer = std::chrono::steady_clock::duration::zero();
            auto usedtime = std::chrono::steady_clock::duration::zero();

            mGrid.reset(config_width, config_height);
            mGrid.generate(2);

            int k = 0;

            while(true) {
                bool gen = false;

                auto beg = std::chrono::steady_clock::now();

                werase(mWin);

                // 绘制分数
                {
                    auto score_str = std::to_string(mGrid.score());
                    auto score_prefix = "Score: ";
                    int score_str_l = get_string_width(score_str);
                    int score_prefix_l = get_string_width(score_prefix);
                    int xpos = CALC_CENTER_BEGIN(getmaxx(mWin), score_str_l + score_prefix_l);
                    int ypos = 2; 
                    mvwaddstr(mWin, ypos, xpos, score_prefix);
                    wattron(mWin, COLOR_PAIR(PAIR_GREEN_TEXT));
                    mvwaddstr(mWin, ypos, xpos + score_prefix_l, score_str.c_str());
                    wattroff(mWin, COLOR_PAIR(PAIR_GREEN_TEXT));
                }

                // 绘制时间
                {
                    int seconds = std::chrono::duration_cast<std::chrono::seconds>(timer).count();
                    int minutes = i32(seconds / 60);
                    seconds %= 60;

                    std::string text("Used time: ");
                    text += std::to_string(minutes) + "分" + std::to_string(seconds) + "秒";
                    waddstrcenter(mWin, 3, text.c_str());

                    timer += frametime;
                }

                if( dbg ) {
                    std::string text("FrameTime=");
                    text += std::to_string(std::chrono::duration_cast<std::chrono::microseconds>(usedtime).count()) + "微秒";
                    waddstrcenter(mWin, getmaxy(mWin) - 3, text.c_str());
                }

                draw_grid();

                nodelay(mWin, 1);
                k = wgetch(mWin);
                nodelay(mWin, 0);

                bool valid;
                switch(k) {
                case KEY_UP:
                    mGrid.merge(DIRECTION::UP, &valid);
                    if( valid )
                        gen = true;
                    break;
                case KEY_DOWN:
                    mGrid.merge(DIRECTION::DOWN, &valid);
                    if( valid )
                        gen = true;
                    break;
                case KEY_RIGHT:
                    mGrid.merge(DIRECTION::RIGHT, &valid);
                    if( valid )
                        gen = true;
                    break;
                case KEY_LEFT:
                    mGrid.merge(DIRECTION::LEFT, &valid);
                    if( valid )
                        gen = true;
                    break;
                case 'q': 
                {
                    bool lcond = true;
                    while(lcond) {
                        const char *msg = "确认退出？Y/n";
                        int msgw = get_string_width(msg);
                        int x = CALC_CENTER_BEGIN(getmaxx(mWin), msgw + 2);
                        int y = CALC_CENTER_BEGIN(getmaxy(mWin), 3);
                        wattron(mWin, COLOR_PAIR(PAIR_DIALOG));
                        wfill(mWin, x, y, x + msgw + 1, y + 2, " ");
                        waddstrcenter(mWin, y + 1, msg);
                        wattroff(mWin, COLOR_PAIR(PAIR_DIALOG));
                        wrefresh(mWin);
                        switch(getch()) {
                        case 'y': case 'Y':
                            cond = false;
                            lcond = false;
                            break;
                        case 'n': case 'N':
                            lcond = false;
                            break;
                        }
                    }
                    break;
                }
                case '\x04':
                    dbg = !dbg;
                    break;
                } // switch(k)

                wrefresh(mWin);

                if( !cond ) {
                    return;
                }

                if( gen ) {
                    mGrid.generate_randomly();
                    if( mGrid.is_fail() ) {
                        cond = false;
                        throw GameOver(mGrid.score(), "莫得可以合并的格子了!", timer);
                    }
                }

                auto end = std::chrono::steady_clock::now();
                usedtime = end - beg;
                if( usedtime < frametime )
                    std::this_thread::sleep_for(frametime - usedtime);
            }
        } // void render_game()

        void render_gameover(const GameOver &gg) {
            static constexpr int DIALOG_H = 10;
            static constexpr const char *TITLE = "游戏结束！";
            auto why = gg.what();
            auto score_str = std::to_string(gg.score());

            int width = get_max(get_string_width(why) + 5, 21, int(score_str.size())) + 2;

            int k = 0;

            bool run_flag = true;

            while(run_flag) {
                int xpos_orig = CALC_CENTER_BEGIN(getmaxx(mWin), width);
                int ypos_orig = CALC_CENTER_BEGIN(getmaxy(mWin), DIALOG_H);

                werase(mWin);

                wattron(mWin, COLOR_PAIR(PAIR_DIALOG));
                wfill(mWin, xpos_orig, ypos_orig, xpos_orig + width - 1, ypos_orig + DIALOG_H - 1, " ");

                waddstrcenter(mWin, ypos_orig + 1, TITLE);
                waddstrcenter(mWin, ypos_orig + 2, (std::string("Why: ") + why).c_str());
                waddstrcenter(mWin, ypos_orig + 4, "分数");
                waddstrcenter(mWin, ypos_orig + 5, score_str.c_str());
                waddstrcenter(mWin, ypos_orig + 7, "按下R查看最后游戏界面");
                waddstrcenter(mWin, ypos_orig + 8, "按下Q退出");

                wrefresh(mWin);

                wattroff(mWin, COLOR_PAIR(PAIR_DIALOG));

                k = getch();
                switch(k) {
                case 'R': case 'r':
                    while(true) {
                        werase(mWin);
                        draw_grid();
                        waddstrcenter(mWin, getmaxy(mWin) - 2, "按下Q退出查看");
                        wrefresh(mWin);
                        int k = getch();
                        if( k == 'q' || k == 'Q' ) {
                            break;
                        }
                    }
                    break;
                case 'Q': case 'q':
                    run_flag = false;
                    break;
                }
            }
        } // void render_gameover()

        void draw_number(int gx, int gy, int global_xcoord, int global_ycoord, u64 nbr) {
            auto &size = config_size;
            auto &fix_rect = config_fix_rect;
            int xsize = fix_rect ? size * 2 : size;

            std::string str = std::to_string(nbr);
            int len = str.size();
            if( !len )
                return;

            int xpos_orig = global_xcoord + 1 + gx * (size + 1) * (fix_rect ? 2 : 1);
            int ypos_orig = global_ycoord + 1 + gy * (size + 1);

            int lines = int(len / xsize) + 1;
            int lastline = len % xsize;

            int ybeg = CALC_CENTER_BEGIN(size, lines);
            int last_xcoord = CALC_CENTER_BEGIN(xsize, lastline);

            // 绘制前几行
            for(int y = 0; y < lines - 1; y++) {
                for(int x = 0; x < size; x++) {
                    char ch = str.front();
                    str.erase(0);
                    mvwaddch(mWin, ypos_orig + ybeg + y, xpos_orig + x, ch);
                }
            }

            // 绘制最后一行
            mvwaddstr(mWin, ypos_orig + ybeg + lines - 1, xpos_orig + last_xcoord, str.c_str());
        } // void draw_number()

        void draw_grid(const Grid &grid) {
            auto &size = config_size;

            // 网格的存储尺寸
            int gwidth = grid.width();
            int gheight = grid.height();

            // 网格的显示尺寸
            int width = gwidth * size + gwidth + 1;
            int height = gheight * size + gheight + 1;

            // 网格左上角相对于WINDOW的偏移
            int global_xcoord = CALC_CENTER_BEGIN(getmaxx(mWin), width + (config_fix_rect ? gwidth * size : 0));
            int global_ycoord = CALC_CENTER_BEGIN(getmaxy(mWin), height);

            // 绘制边框
            for( int y = 0; y < height; y++ ) {
                int xcoord = 0;
                for( int x = 0; x < width; x++ ) {
                    int xpos = global_xcoord + x + xcoord;
                    int ypos = global_ycoord + y;
                    const char *str = nullptr;
                    if( y == 0 ) {
                        if( x == 0 ) {
                            str = "╔";
                        } else if( x == width - 1 ) {
                            str = "╗";
                        } else if( x % (size + 1) == 0 ) {
                            str = "╦";
                        } else {
                            if( config_fix_rect ) {
                                str = "══";
                                xcoord += 1;
                            } else {
                                str = "═";
                            }
                        }
                    } else if( y == height - 1 ) {
                        if( x == 0 ) {
                            str = "╚";
                        } else if( x == width - 1 ) {
                            str = "╝";
                        } else if( x % (size + 1) == 0 ) {
                            str = "╩";
                        } else {
                            if( config_fix_rect ) {
                                str = "══";
                                xcoord += 1;
                            } else {
                                str = "═";
                            }
                        }
                    } else if( y % (size + 1) == 0 ) {
                        if( x == 0 ) {
                            str = "╠";
                        } else if( x == width - 1 ) {
                            str = "╣";
                        } else if( x % (size + 1) == 0 ) {
                            str = "╬";
                        } else {
                            if( config_fix_rect ) {
                                str = "══";
                                xcoord += 1;
                            } else {
                                str = "═";
                            }
                        }
                    } else {
                        if( x == 0 ) {
                            str = "║";
                        } else if( x == width - 1 ) {
                            str = "║";
                        } else if( x % (size + 1) == 0 ) {
                            str = "║";
                        } else {
                            if( config_fix_rect ) {
                                str = "  ";
                                xcoord += 1;
                            } else {
                                str = " ";
                            }
                        }
                    }
                    mvwaddstr(mWin, ypos, xpos, str);
                } // for( int x = 0; x < width; x++ )
            } // for( int y = 0; y < height; y++ )

            // 绘制表格内容，0表示为空格子
            for( int x = 0; x < grid.width(); x++ ) {
                for( int y = 0; y < grid.height(); y++ ) {
                    auto v = grid.get(x, y);
                    if( v != 0 ) {
                        draw_number(x, y, global_xcoord, global_ycoord, v);
                    }
                } // for( int y = 0; y < grid.height(); y++ )
            } // for( int x = 0; x < grid.width(); x++ )
        } // void draw_grid()

        void draw_grid() {
            draw_grid(mGrid);
        }

        void render_settings() {
            waddstrcenter(mWin, int(getmaxy(mWin) * 0.5), "In development!");
            waddstrcenter(mWin, int(getmaxy(mWin) * 0.5) + 1, "Press any key to back");
            getch();
        } // void render_settings()

        /// Always throw a GameStop exception with code 0 message "Normally exit"
        void stop() {
            throw GameStop(0, "Normally exit");
        } // void stop()

        void run() {
            while(true) {
                try {
                    render_title();
                } catch(GameOver &gg) {
                    render_gameover(gg);
                }
            }
        } // void run()

        int config_width;
        int config_height;
        int config_size;
        bool config_fix_rect;

    private:
        WINDOW *mWin;
        Grid mGrid;
        int mEasterStatus;
    };




}

int main() {
    setlocale(LC_ALL, "");

    initscr();

    x2048::Game game;

    try {
        game.run();
    } catch(x2048::GameStop &stop_msg) {
        endwin();
        std::cerr << "[X2048] Game exited";
        if( stop_msg.code() != 0 )
            std::cerr << " with code " + std::to_string(stop_msg.code());
        std::cerr << std::endl;
        if( stop_msg.code() != 0 )
            std::cerr << "[X2048] Message: " << stop_msg.what() << std::endl;
    } catch(std::exception &err) {
        endwin();
        std::cerr << err.what() << std::endl;
    } catch(...) {
        endwin();
        std::cerr << "[X2048] GAME STOPPED DUE TO AN UNKNOWN ERROR THOWN!" << std::endl;
    }
    endwin();
    return 0;
}
