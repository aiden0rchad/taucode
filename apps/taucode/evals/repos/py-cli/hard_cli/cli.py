import argparse
import json
import sys
from .reader import read_inputs
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--limit',type=int)
    parser.add_argument('paths',nargs='*')
    args=parser.parse_args()
    rows=read_inputs(args.paths,sys.stdin)
    print(json.dumps(rows[:args.limit]))
if __name__ == '__main__':
    main()
