import random

i = 114515
target = random.randint(0, 1000)
count = 0
max = 20
while count <= max:
    try:
        i = int(input(f'还有{max - count}次只因会 Input: '))
    except ValueError:
        print('tmd输数字')
        continue
    if i != target:
        if i < target:
            print('Too 小')
        elif i > target:
            print('Too 大')
        count += 1
        continue
    else:
        print('You win!')
        print(f'总共使用了{count}次机会')
        break