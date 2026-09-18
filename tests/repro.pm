MOD repro:
    # Repro module to test whether function bodies are executed at import time

    DEF test():
        d = {}
        d["x"] = 1
        RAISE("boom")
    END

    DEF exports():
        RETURN(["test"])
    END
