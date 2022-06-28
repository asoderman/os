import os, sys

def main():
    arg = sys.argv[1]
    replacement_file = []
    with open('Cargo.toml', 'r') as f:
        for line in f.readlines():
            if line.startswith('crate-type'):
                line = f'crate-type = ["{arg}"]\n'
            replacement_file.append(line)

    with open('Cargo.toml', 'w') as f:
        f.writelines(replacement_file)


if __name__ == '__main__':
    main()
