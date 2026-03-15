# stdlib/tensor.ph — Tensor system
#
# Literal syntax:   BUILD TENSOR: [[1,2],[3,4]]
#
# Constructors:  tensor.zeros(shape)  tensor.ones(shape)  tensor.rand(shape)
#                tensor.eye(n)        tensor.from_list(list)  tensor.fill(shape,v)
# Queries:       tensor.shape(t)      tensor.dtype(t)
# Reductions:    tensor.sum(t)        tensor.mean(t)
# Transform:     tensor.reshape(t,sh) tensor.transpose(t)  tensor.flatten(t)
# Utility:       tensor.clone(t)      tensor.to_list(t)
# Arithmetic:    tensor.add(a,b)      tensor.sub(a,b)
#                tensor.mul(a,b)      tensor.div(a,b)  (element-wise)

set __header_tensor = "tensor loaded"
set tensor.pi = 3.141592653589793

DEF tensor.zeros(shape):        RET.NOW(): tensor.zeros(shape)          END
DEF tensor.ones(shape):         RET.NOW(): tensor.ones(shape)           END
DEF tensor.rand(shape):         RET.NOW(): tensor.rand(shape)           END
DEF tensor.eye(n):              RET.NOW(): tensor.eye(n)                END
DEF tensor.from_list(lst):      RET.NOW(): tensor.from_list(lst)        END
DEF tensor.fill(shape, v):      RET.NOW(): tensor.fill(shape, v)        END
DEF tensor.shape(t):            RET.NOW(): tensor.shape(t)              END
DEF tensor.dtype(t):            RET.NOW(): tensor.dtype(t)              END
DEF tensor.sum(t):              RET.NOW(): tensor.sum(t)                END
DEF tensor.mean(t):             RET.NOW(): tensor.mean(t)               END
DEF tensor.reshape(t, sh):      RET.NOW(): tensor.reshape(t, sh)        END
DEF tensor.transpose(t):        RET.NOW(): tensor.transpose(t)          END
DEF tensor.flatten(t):          RET.NOW(): tensor.flatten(t)            END
DEF tensor.clone(t):            RET.NOW(): tensor.clone(t)              END
DEF tensor.to_list(t):          RET.NOW(): tensor.to_list(t)            END
DEF tensor.add(a, b):           RET.NOW(): tensor.add(a, b)             END
DEF tensor.sub(a, b):           RET.NOW(): tensor.sub(a, b)             END
DEF tensor.mul(a, b):           RET.NOW(): tensor.mul(a, b)             END
DEF tensor.div(a, b):           RET.NOW(): tensor.div(a, b)             END
