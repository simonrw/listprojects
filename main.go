package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"io/ioutil"
	"log"
	"os"
	"os/user"
	"path"
	"path/filepath"
	"strings"

	"github.com/adrg/xdg"
	"github.com/jessevdk/go-flags"
	"github.com/ktr0731/go-fuzzyfinder"
	"github.com/pelletier/go-toml/v2"
	"gitlab.com/srwalker101/project/projectpath"
	"gitlab.com/srwalker101/project/tmux"
)

type Config struct {
	RootDirs []RootDir `toml:"root_dirs"`
}

type RootDir struct {
	Path   string `toml:"path"`
	Prefix string `toml:"prefix" omitempty:"true"`
}

// expandUser replaces "~" characters with the users home directory
func expandUser(p string) string {
	if !strings.HasPrefix(p, "~") {
		return p
	}

	usr, _ := user.Current()
	dir := usr.HomeDir

	return filepath.Join(dir, p[2:])

}

func OpenConfig() (*Config, error) {
	p := path.Join(xdg.ConfigHome, "project", "config.toml")
	b, err := ioutil.ReadFile(p)
	if err != nil {
		return nil, fmt.Errorf("opening config file: %w", err)
	}
	var c Config
	if err = toml.Unmarshal(b, &c); err != nil {
		return nil, fmt.Errorf("reading config file: %w", err)
	}
	// expand out ~ characters
	for i, r := range c.RootDirs {
		r.Path = expandUser(r.Path)
		c.RootDirs[i] = r
	}

	return &c, nil
}

// helper functions
func fileExists(path string) bool {
	s, err := os.Stat(path)
	if errors.Is(err, os.ErrNotExist) {
		return false
	}
	return !s.IsDir()
}

func dirExists(path string) bool {
	s, err := os.Stat(path)
	if errors.Is(err, os.ErrNotExist) {
		return false
	}
	if s == nil {
		return false
	}
	return s.IsDir()
}

type Cache struct {
	Paths map[projectpath.Path]bool
	loc   string
}

type serializedCache struct {
	Paths []projectpath.Path `json:"paths"`
}

func (c Cache) toSerialized() serializedCache {
	paths := c.InitialPaths()
	return serializedCache{Paths: paths}
}

func (s serializedCache) fromSerialized(path string) Cache {
	c := Cache{
		loc: path,
	}
	c.Paths = make(map[projectpath.Path]bool)
	for _, p := range s.Paths {
		c.Add(p)
	}

	return c
}

func New(clear bool) (*Cache, error) {
	cacheDir := path.Join(xdg.CacheHome, "project")
	os.MkdirAll(cacheDir, 0700)
	cachePath := path.Join(cacheDir, "config.json")

	if !fileExists(cachePath) {
		c := Cache{loc: cachePath}
		c.Paths = make(map[projectpath.Path]bool)
		return &c, nil
	}

	cacheData, err := ioutil.ReadFile(cachePath)
	if err != nil {
		return nil, fmt.Errorf("could not read cache file: %w", err)
	}

	var s serializedCache
	if err = json.Unmarshal(cacheData, &s); err != nil {
		return nil, fmt.Errorf("could not read cache content: %w", err)
	}
	cache := s.fromSerialized(cachePath)

	if clear {
		cache.Paths = make(map[projectpath.Path]bool)
	}
	cache.Write()

	return &cache, nil
}

func (c Cache) InitialPaths() []projectpath.Path {
	var paths []projectpath.Path
	for p := range c.Paths {
		paths = append(paths, p)
	}
	return paths
}

func (c *Cache) Write() {
	s := c.toSerialized()
	contents, err := json.Marshal(s)
	if err != nil {
		panic(err)
	}
	if err = ioutil.WriteFile(c.loc, contents, 0600); err != nil {
		panic(err)
	}
}

func (c *Cache) Add(entry projectpath.Path) {
	c.Paths[entry] = true
}

func (c Cache) Contains(entry projectpath.Path) bool {
	return c.Paths[entry]
}

func sessionName(root RootDir, path string) string {
	return root.Prefix + strings.TrimLeft(strings.TrimPrefix(path, root.Path), "/")
}

func main() {

	var opts struct {
		Clear bool `short:"c" long:"clear" description:"clear cache"`
	}

	_, err := flags.Parse(&opts)
	if err != nil {
		log.Fatal(err)
	}

	cfg, err := OpenConfig()
	if err != nil {
		log.Fatalf("could not open config file: %v", err)
	}

	cache, err := New(opts.Clear)
	if err != nil {
		log.Fatal(err)
	}
	defer cache.Write()

	paths := cache.InitialPaths()

	for _, rootDir := range cfg.RootDirs {
		go func(r RootDir) {
			if err := filepath.WalkDir(r.Path, func(p string, d fs.DirEntry, err error) error {
				if dirExists(path.Join(p, ".git")) {
					entry := projectpath.Path{
						FullPath:    p,
						SessionName: sessionName(r, p),
					}
					if !cache.Contains(entry) {
						paths = append(paths, entry)
						cache.Add(entry)
					}
					return filepath.SkipDir
				}
				return nil
			}); err != nil {
				panic(err)
			}
		}(rootDir)
	}

	idx, err := fuzzyfinder.Find(
		&paths,
		func(i int) string {
			return paths[i].FullPath
		},
		fuzzyfinder.WithHotReload(),
		fuzzyfinder.WithHeader("Choose project"),
	)

	if err != nil {
		if errors.Is(err, fuzzyfinder.ErrAbort) {
			// exit with failed status code so that the tmux binding does not kill the current session
			cache.Write()
			os.Exit(1)
		} else {
			log.Fatal(err)
		}
	}
	// set up the tmux session
	selectedPath := paths[idx]
	session := tmux.NewSession(selectedPath)
	if err := session.Switch(); err != nil {
		panic(err)
	}
}
