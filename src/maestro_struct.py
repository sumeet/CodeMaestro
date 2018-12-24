class MaestroStruct:

    def __init__(self, name, struct_dict):
        self._struct_dict = struct_dict
        self._name = name

    def __repr__(self):
        return f'<{self._name}: {repr(self._struct_dict)}>'

    def __getattr__(self, name):
        if name not in self._struct_dict:
            raise NameError
        return self._struct_dict[name]
