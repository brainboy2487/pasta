# Makefile - improved build for Mage
# Usage:
#   make            -> build lib/libmage.so
#   make -j8        -> parallel build
#   make clean      -> remove build artifacts
#   make distclean  -> remove build, bin, lib
#   make install    -> install lib and headers (adjust PREFIX)

# Toolchain
CC ?= gcc
AR ?= ar
RM ?= rm -rf
MKDIR_P ?= mkdir -p

# Paths
SRCDIR := src
BUILDDIR := build
LIBDIR := lib
BINDIR := bin
INCDIRS := -Iinclude -I/usr/include

# Compiler flags
CFLAGS ?= -Wall -Wextra -O3 -march=native -std=c11 -fPIC $(INCDIRS) -DHAVE_UTF8PROC
# Keep -fPIC for shared lib; adjust optimization for debugging if needed
# Linker flags and libs
LDFLAGS ?= -shared
LIBS ?= -lm -lpthread -ldl -lutf8proc

# Source discovery
# Collect all .c files under src/ recursively
SRC := $(shell find $(SRCDIR) -name '*.c' | sort)

# Detect duplicate API provider: if core/mage_parser.c defines mage_api_tokenize
# then skip building src/utils/token_api.c to avoid multiple-definition at link time.
# This is a pragmatic workaround for the current codebase where both files provide the same symbols.
SKIP_TOKEN_API := $(shell if [ -f src/core/mage_parser.c ] && grep -q "mage_api_tokenize" src/core/mage_parser.c 2>/dev/null; then echo 1; else echo 0; fi)

ifeq ($(SKIP_TOKEN_API),1)
  # Remove token_api.c from the source list if present
  SRC := $(filter-out src/utils/token_api.c,$(SRC))
  SKIP_MSG := "Skipping src/utils/token_api.c (symbols provided by src/core/mage_parser.c)"
else
  SKIP_MSG :=
endif

# Convert source list to object list under build/
OBJ := $(patsubst $(SRCDIR)/%.c,$(BUILDDIR)/%.o,$(SRC))

# Ensure build subdirs exist for object files
# Extract unique directories from OBJ
OBJDIRS := $(sort $(dir $(OBJ)))

# Output targets
LIB := $(LIBDIR)/libmage.so

.PHONY: all clean distclean install dirs show-config

all: dirs show-config $(LIB)

# Print configuration for debugging
show-config:
	@echo "CC=$(CC)"
	@echo "CFLAGS=$(CFLAGS)"
	@echo "LDFLAGS=$(LDFLAGS)"
	@echo "LIBS=$(LIBS)"
	@if [ -n "$(SKIP_MSG)" ]; then echo $(SKIP_MSG);
	fi
		@echo "Sources: $(words $(SRC)) files"
		@echo "Objects: $(words $(OBJ)) files"

# Create build and output directories
dirs:
	@$(MKDIR_P) $(BUILDDIR)
	@for d in $(OBJDIRS); do $(MKDIR_P) $$d; done
	@$(MKDIR_P) $(LIBDIR) $(BINDIR)

# Pattern rule to compile .c -> .o
# preserves directory structure under build/
$(BUILDDIR)/%.o: $(SRCDIR)/%.c
	@echo "CC $< -> $@"
	@$(CC) $(CFLAGS) -c -o $@ $<

# Link shared library
$(LIB): $(OBJ)
	@echo "LINK $@"
	@$(CC) $(LDFLAGS) -o $@ $(OBJ) $(LIBS)
	@echo "Built $@"

# Convenience: build static archive as well (optional)
$(LIBDIR)/libmage.a: $(OBJ)
	@$(MKDIR_P) $(LIBDIR)
	@echo "AR $@"
	@$(AR) rcs $@ $(OBJ)

# Install (adjust PREFIX if needed)
PREFIX ?= /usr/local
install: all
	@echo "Installing lib to $(PREFIX)/lib and headers to $(PREFIX)/include"
	@install -d $(PREFIX)/lib
	@install -d $(PREFIX)/include
	@install -m 0644 $(LIB) $(PREFIX)/lib/
	@# copy headers (if you want to install headers, adjust as needed)
	@cp -r include/* $(PREFIX)/include/ 2>/dev/null || true
	@echo "Install complete"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	# @$(RM) $(BUILDDIR)
	@$(RM) $(LIBDIR)/*.a 2>/dev/null || true
	@$(RM) $(BINDIR)/* 2>/dev/null || true

# Remove everything including lib and bin
distclean: clean
	@echo "Removing lib and bin directories..."
	@$(RM) $(LIBDIR) $(BINDIR)

# Developer helper: show which token_api decision was made
show-skip:
	@if [ "$(SKIP_TOKEN_API)" = "1" ]; then echo "token_api.c skipped (mage_parser provides same symbols)"; else echo "token_api.c included in build"; fi

# Allow overriding default targets
.DEFAULT_GOAL := all
