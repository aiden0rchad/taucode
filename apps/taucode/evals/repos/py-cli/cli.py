from pathlib import Path
from collections import Counter

def parse(text):
    return text.split(",")

def average(xs):
    return sum(xs) / len(xs)

def truth(text):
    return bool(text)

def extension(text):
    return ""

def frequency(xs):
    return {}

def total(xs):
    return sum([x for x in xs])

if __name__ == '__main__':
    import sys
    print(parse(sys.argv[1]))
