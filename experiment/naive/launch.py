import sys, os
import inspect
import time

sys.path.insert(0, "/workspace")

from pipeline import NaivePipeline
import pathlib

def generate_config(src, dst, **kwargs):
    with open(src) as fsrc, open(dst, 'w') as fdst:
        for line in fsrc:
            fdst.write(line.format(**kwargs))


if __name__ == "__main__":
    import argparse
    import yaml

    parser = argparse.ArgumentParser(
            formatter_class=argparse.ArgumentDefaultsHelpFormatter
    )
    parser.add_argument("input_path")
    parser.add_argument("output_dir")
    args = vars(parser.parse_args())

    # Create output directory
    output_dir = pathlib.Path(args['output_dir'])
    output_dir.mkdir(exist_ok=True, parents=True)

    # Output results into dnn.csv file
    args['output_path'] = str(output_dir / 'dnn.csv')

    # Generate config file for the pipeline
    new_config = output_dir / 'config.yaml'
    generate_config('config.yaml', new_config, **args)

    with open(output_dir / 'log.txt', 'w+') as f:
        # Load new config file and start pipeline
        config = yaml.safe_load(open(new_config))
        pipeline = NaivePipeline(config, f=f)
        print("Created pipeline")
        pipeline.start()

