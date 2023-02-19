import random

length = 6
cur_length = 0
errors_c = 0

numset = [str(i) for i in range(0, 10)]
num = []

max_count = 10
count = max_count

def randitem(l):
    i = random.randint(0, len(l) - 1)
    v = l[i]
    del(l[i])
    return v

while cur_length < length:
    num.append(randitem(numset))
    cur_length += 1

print('Guess?')
print('输入help获取游戏规则')
while count:
    if errors_c == 20:
        print('-- 你tm故意耍我是吧?')
        print('-- 萨日朗!!!')
        errors_c = 21
    
    i_v = input(f'剩余{count}次只因会: ')
    
    if i_v == 'help':
        print('输入4位数字，输出绿色为位置和数字正确，蓝色为位置错误，根据提示，猜出原来的数字')
        continue
    
    if len(i_v) != length:
        errors_c += 1
        print('长度不对')
        continue
    
    allcorrect = True
    for i in range(0, length):
        if i_v[i] == num[i]:
            print('\x1b[32m', end='', flush=False)
        elif i_v[i] in num:
            print('\x1b[34m', end='', flush=False)
            allcorrect = False
        else:
            allcorrect = False
        print(i_v[i], end='', flush=False)
        print('\x1b[0m', end='', flush=False)
    print('')
    
    if allcorrect:
        print('Wooo, you just won the game!')
        print(f'一共用了{max_count - count}次机会!')
        print('太逊了' if count / max_count < 0.2 else '还行吧')
        break
    
    count -= 1