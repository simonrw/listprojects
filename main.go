package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"io/ioutil"
	"log"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"strings"

	"github.com/adrg/xdg"
	"github.com/jessevdk/go-flags"
	"github.com/ktr0731/go-fuzzyfinder"
)

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

type Path struct {
	FullPath string
}

type Cache struct {
	Paths map[Path]bool
	loc   string
}

type serializedCache struct {
	Paths []Path `json:"paths"`
}

func (c Cache) toSerialized() serializedCache {
	paths := c.InitialPaths()
	return serializedCache{Paths: paths}
}

func (s serializedCache) fromSerialized(path string) Cache {
	c := Cache{
		loc: path,
	}
	c.Paths = make(map[Path]bool)
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
		return &Cache{loc: cachePath}, nil
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
		cache.Paths = make(map[Path]bool)
	}
	cache.Write()

	return &cache, nil
}

func (c Cache) InitialPaths() []Path {
	var paths []Path
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

func (c *Cache) Add(entry Path) {
	c.Paths[entry] = true
}

func (c Cache) Contains(entry Path) bool {
	return c.Paths[entry]
}

// Session wraps the functionality of tmux
type Session struct {
	path string
}

func NewSession(path string) *Session {
	return &Session{path}
}

func (s Session) TmuxRunning() bool {
	return os.Getenv("TMUX") != ""
}

func (s Session) Exists() bool {
	var buf bytes.Buffer
	cmd := exec.Command("tmux", "ls", "-F", "#S")
	cmd.Stdout = &buf
	err := cmd.Run()
	if err != nil {
		log.Printf("error spawning command: %v", err)
	}

	output := strings.Split(buf.String(), "\n")
	for _, session := range output {
		if session == s.sessionName() {
			return true
		}
	}
	return false
}

func (s Session) SwitchClient() {
}

func (s Session) CreateSession() {
}

func (s Session) Create() {
}

func (s Session) Join() {
}

func (s Session) sessionName() string {
	return s.path
}

func main() {

	var opts struct {
		Clear bool `short:"c" long:"clear" description:"clear cache"`
	}

	_, err := flags.Parse(&opts)
	if err != nil {
		log.Fatal(err)
	}

	cache, err := New(opts.Clear)
	if err != nil {
		log.Fatal(err)
	}
	defer cache.Write()

	paths := cache.InitialPaths()

	go func() {
		if err := filepath.WalkDir("/home/simon/dev", func(p string, d fs.DirEntry, err error) error {
			if dirExists(path.Join(p, ".git")) {
				entry := Path{
					FullPath: p,
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
	}()

	idx, err := fuzzyfinder.Find(
		&paths,
		func(i int) string {
			return paths[i].FullPath
		},
		fuzzyfinder.WithHotReload(),
		fuzzyfinder.WithHeader("Choose project"),
	)

	if err != nil {
		log.Fatal(err)
	}
	// set up the tmux session
	selectedPath := paths[idx]
	session := NewSession(selectedPath.FullPath)
	if session.TmuxRunning() {
		if session.Exists() {
			session.SwitchClient()
		} else {
			session.CreateSession()
			session.SwitchClient()
		}
	} else {
		session.Create()
		session.Join()
	}
}