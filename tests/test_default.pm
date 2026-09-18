MOD testdefault:
    export dumps

    DEF dumps(v, indent=None):
        IF indent != None:
            RETURN("has indent")
        END
        RETURN("no indent")
    END

    DEF exports():
        RETURN(["dumps"])
    END
