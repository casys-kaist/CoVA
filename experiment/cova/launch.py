import sys, os
import inspect
import time

sys.path.insert(0, "/workspace")

from pipeline import CovaPipeline
import pathlib
import socketserver

import signal

# Aggregator command
BUILD = 'release' # 'debug'
AGG_CMD = '/workspace/cova-rs/target/{build}/analysis-aggregator {output_dir} {track_port} {dnn_port} --num-tracker {num_entdec} --scale-factor {scale_factor} --moving-iou {moving_iou} --stationary-iou {stationary_iou} --stationary-maxage {stationary_maxage}'

# Terminate aggregator
agg = None
def handler(signum, frame):
    global agg
    if agg is not None:
        agg.terminate()
    exit(0)

signal.signal(signal.SIGINT, handler)

def generate_config(src, dst, **kwargs):
    with open(src) as fsrc, open(dst, 'w') as fdst:
        for line in fsrc:
            fdst.write(line.format(**kwargs))


if __name__ == "__main__":
    import argparse
    import yaml
    import subprocess
    import shlex

    parser = argparse.ArgumentParser()
    parser.add_argument("input_path")
    parser.add_argument("output_dir")
    parser.add_argument("dataset")
    parser.add_argument("--maxage", nargs='?', default=60)
    parser.add_argument("--minhit", nargs='?', default=30)
    parser.add_argument("--cuda", nargs='?', default=0)
    parser.add_argument("--perf", action='store_true')
    parser.add_argument("--moving-iou", default=0.1)
    parser.add_argument("--stationary-iou", default=0.5)
    parser.add_argument("--stationary-maxage", default=60)
    parser.add_argument("--scale-factor", default=1.4)
    args = vars(parser.parse_args())

    if args['perf']:
        track_port = 0
        dnn_port = 0
    else:
        # Find free port, does not garantee they will remain free and might fail
        with socketserver.TCPServer(("localhost", 0), None) as s1:
            with socketserver.TCPServer(("localhost", 0), None) as s2:
                track_port = s1.server_address[1]
                dnn_port = s2.server_address[1]


    args['track_port'] = track_port
    args['dnn_port'] = dnn_port

    # Create output directory
    output_dir = pathlib.Path(args['output_dir'])
    output_dir.mkdir(exist_ok=True, parents=True)

    # Generate config file for the pipeline
    new_config = output_dir / 'config.yaml'
    generate_config('config.yaml', new_config, **args)

    # Load new config file
    config = yaml.safe_load(open(new_config))

    if not args['perf']:
        # Run aggregator
        os.environ["CUDA_VISIBLE_DEVICES"] = str(args['cuda'])
        cmd = AGG_CMD.format(build=BUILD, **args, **config)
        print('Running:', cmd)
        agg = subprocess.Popen(shlex.split(cmd))

    # Start pipeline
    with open(output_dir / 'out.txt', 'w+') as f:
        pipeline = CovaPipeline(config, f=f)
        print("Created Pipeline")
        pipeline.start()

    if not args['perf']:
        # Terminate aggregator
        agg.wait(timeout=60)
