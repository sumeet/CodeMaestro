from collections import namedtuple
import operator
import json
import pprint



Bool = namedtuple('Bool', '')
String = namedtuple('String', '')
Int = namedtuple('Int', '')
Float = namedtuple('Float', '')
Null = namedtuple('Null', '')
List = namedtuple('List', 'of')
Dict = namedtuple('Dict', 'value_by_key')
EmptyCantInfer = namedtuple('EmptyCantInfer', '')
NonHomogeneousCantParse = namedtuple('NonHomogeneousCantParse', '')


def infer_type(j):
    if isinstance(j, list):
        types = map(infer_type, j)
        types = set(t for t in types if not isinstance(t, EmptyCantInfer))
        # if there's no inferrable types in here, then we also cannot infer the
        # type here.
        if not types:
            return EmptyCantInfer()
        if len(types) > 1:
            return NonHomogeneousCantParse()
        return List(types.pop())

    if isinstance(j, dict):
        return Dict(tuple(sorted(((key, infer_type(value)) for key, value in j.items()),
                                 key=operator.itemgetter(0))))

    if j is None:
        return Null()

    if isinstance(j, str):
        return String()

    if isinstance(j, int):
        return Int()

    if isinstance(j, bool):
        return Bool()

    if isinstance(j, float):
        return Float()


import sys
resp = json.load(open(sys.argv[1]))
pprint.pprint(infer_type(resp))
