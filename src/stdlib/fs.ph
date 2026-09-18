# stdlib/fs.ph — File system access
#
# Exports:
#   fs.read(path)          -> string
#   fs.write(path, data)
#   fs.append(path, data)
#   fs.exists(path)        -> bool
#   fs.delete(path)
#   fs.list(dir)           -> list of filenames
#   fs.mkdir(path)
#   fs.rmdir(path)
#   fs.size(path)          -> number (bytes)
#   fs.is_dir(path)        -> bool
#   fs.is_file(path)       -> bool
#   fs.copy(src, dst)
#   fs.move(src, dst)
#   fs.realpath(path)      -> string (absolute)
#   fs.getcwd()            -> string
#   fs.touch(path)         create empty file if not exists
#   fs.basename(path)      -> string (last component)
#   fs.dirname(path)       -> string (parent directory)
#   fs.ext(path)           -> string (extension without dot)

set __header_fs = "fs loaded"

DEF fs.read(path):        RET.NOW(): fs.read(path)         END
DEF fs.write(path, data): fs.write(path, data)             END
DEF fs.append(path, data): fs.append(path, data)           END
DEF fs.exists(path):      RET.NOW(): fs.exists(path)       END
DEF fs.delete(path):      fs.delete(path)                  END
DEF fs.list(path):        RET.NOW(): fs.list(path)         END
DEF fs.mkdir(path):       fs.mkdir(path)                   END
DEF fs.rmdir(path):       fs.rmdir(path)                   END
DEF fs.size(path):        RET.NOW(): fs.size(path)         END
DEF fs.is_dir(path):      RET.NOW(): fs.is_dir(path)       END
DEF fs.is_file(path):     RET.NOW(): fs.is_file(path)      END
DEF fs.copy(src, dst):    fs.copy(src, dst)                END
DEF fs.move(src, dst):    fs.move(src, dst)                END
DEF fs.realpath(path):    RET.NOW(): fs.realpath(path)     END
DEF fs.getcwd():          RET.NOW(): fs.getcwd()           END
DEF fs.touch(path):       fs.touch(path)                   END
DEF fs.basename(path):    RET.NOW(): fs.basename(path)     END
DEF fs.dirname(path):     RET.NOW(): fs.dirname(path)      END
DEF fs.ext(path):         RET.NOW(): fs.ext(path)          END
