#!/usr/bin/env python

import os
import json


json_dump_of_env_vars = json.dumps(dict(os.environ), indent=4)
print('ENV = %s;' % json_dump_of_env_vars)
